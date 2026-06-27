//! Filesystem secondary adapter for reading `task-contract.json`.
//!
//! [`FsTaskContractReader`] implements
//! [`usecase::pre_review_gate::TaskContractReaderPort`]. It reads
//! `<items_dir>/<track_id>/task-contract.json`, decodes it via
//! [`super::task_contract_codec::decode`], and returns a
//! `domain::task_contract::TaskContractDocument`.

use std::path::PathBuf;

use domain::TrackId;
use domain::task_contract::TaskContractDocument;
use usecase::pre_review_gate::{PreReviewGateError, TaskContractReaderPort};

use crate::task_contract_codec;
use crate::track::symlink_guard::reject_symlinks_below;

const MAX_TASK_CONTRACT_BYTES: u64 = 1024 * 1024;

/// Filesystem secondary adapter implementing
/// [`usecase::pre_review_gate::TaskContractReaderPort`].
///
/// Reads `<items_dir>/<track_id>/task-contract.json`, decodes it via
/// `task_contract_codec::decode`, and returns a
/// `domain::task_contract::TaskContractDocument`.
///
/// - Missing file maps to [`PreReviewGateError::TaskContractNotFound`].
/// - I/O and codec errors map to [`PreReviewGateError::TaskContractReadFailed`].
///
/// The `items_dir` is injected at construction time so callers do not need to
/// pass it on every [`read`](FsTaskContractReader::read) call.
#[derive(Debug)]
pub struct FsTaskContractReader {
    items_dir: PathBuf,
}

impl FsTaskContractReader {
    /// Construct a `FsTaskContractReader` with the given items directory root.
    #[must_use]
    pub fn new(items_dir: PathBuf) -> Self {
        Self { items_dir }
    }
}

impl TaskContractReaderPort for FsTaskContractReader {
    fn read(&self, track_id: &TrackId) -> Result<TaskContractDocument, PreReviewGateError> {
        let items_dir =
            crate::resolve_items_dir_under_current_repo(&self.items_dir).map_err(|e| {
                PreReviewGateError::TaskContractReadFailed {
                    message: format!("items_dir rejected before reading task-contract.json: {e}"),
                }
            })?;
        let path = items_dir.join(track_id.as_ref()).join("task-contract.json");

        match reject_symlinks_below(&path, &items_dir) {
            Ok(true) => {}
            Ok(false) => return Err(PreReviewGateError::TaskContractNotFound),
            Err(e) => {
                return Err(PreReviewGateError::TaskContractReadFailed {
                    message: format!("symlink check failed for {}: {e}", path.display()),
                });
            }
        }

        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(PreReviewGateError::TaskContractReadFailed {
                    message: format!(
                        "symlink check failed for {}: refused symlink",
                        path.display()
                    ),
                });
            }
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(PreReviewGateError::TaskContractNotFound);
            }
            Err(e) => {
                return Err(PreReviewGateError::TaskContractReadFailed {
                    message: format!("metadata error reading {}: {e}", path.display()),
                });
            }
        };
        if metadata.len() > MAX_TASK_CONTRACT_BYTES {
            return Err(PreReviewGateError::TaskContractReadFailed {
                message: format!(
                    "task-contract.json exceeds maximum size of {MAX_TASK_CONTRACT_BYTES} bytes: {} bytes",
                    metadata.len()
                ),
            });
        }

        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(PreReviewGateError::TaskContractNotFound);
            }
            Err(e) => {
                return Err(PreReviewGateError::TaskContractReadFailed {
                    message: format!("I/O error reading {}: {e}", path.display()),
                });
            }
        };

        let doc = task_contract_codec::decode(&bytes).map_err(|e| {
            PreReviewGateError::TaskContractReadFailed {
                message: format!("codec error reading {}: {e}", path.display()),
            }
        })?;

        // PR #175 round 2 P1: refuse contract whose embedded track_id does not
        // match the requested track_id. Without this, a contract copied from
        // another track (or generated with a stale track id) would silently
        // evaluate attribution for unrelated data.
        if doc.track_id() != track_id {
            return Err(PreReviewGateError::TaskContractReadFailed {
                message: format!(
                    "track_id mismatch: requested '{}' but task-contract.json contains '{}'",
                    track_id.as_ref(),
                    doc.track_id().as_ref()
                ),
            });
        }

        Ok(doc)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;

    use domain::TrackId;
    use usecase::pre_review_gate::PreReviewGateError;

    use super::*;

    fn track_id(s: &str) -> TrackId {
        TrackId::try_new(s).unwrap()
    }

    fn temp_items_dir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("task-contract-reader-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap()
    }

    const SAMPLE_JSON: &str = r#"{
  "schema_version": 1,
  "track_id": "my-track",
  "entries": {
    "T001": [
      {"layer": "domain", "entry_key": "MyType"}
    ]
  }
}"#;

    #[test]
    fn read_returns_document_for_existing_file() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("task-contract.json"), SAMPLE_JSON).unwrap();

        let reader = FsTaskContractReader::new(dir.path().to_path_buf());
        let doc = reader.read(&track_id("my-track")).unwrap();
        assert_eq!(doc.track_id().as_ref(), "my-track");
    }

    #[test]
    fn read_returns_read_failed_for_embedded_track_id_mismatch() {
        // PR #175 round 2 P1: contract whose embedded track_id does not match
        // the requested track_id must fail closed, not silently evaluate
        // attribution for unrelated data.
        let dir = temp_items_dir();
        let track_dir = dir.path().join("other-track");
        fs::create_dir_all(&track_dir).unwrap();
        // SAMPLE_JSON embeds track_id "my-track" but we request "other-track".
        fs::write(track_dir.join("task-contract.json"), SAMPLE_JSON).unwrap();

        let reader = FsTaskContractReader::new(dir.path().to_path_buf());
        let err = reader.read(&track_id("other-track")).unwrap_err();
        match err {
            PreReviewGateError::TaskContractReadFailed { message } => {
                assert!(
                    message.contains("track_id mismatch"),
                    "expected track_id mismatch diagnostic, got: {message}"
                );
                assert!(message.contains("other-track"), "expected requested id in message");
                assert!(message.contains("my-track"), "expected embedded id in message");
            }
            other => panic!("expected TaskContractReadFailed, got: {other}"),
        }
    }

    #[test]
    fn read_returns_not_found_for_missing_file() {
        let dir = temp_items_dir();
        let reader = FsTaskContractReader::new(dir.path().to_path_buf());
        let err = reader.read(&track_id("nonexistent-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::TaskContractNotFound),
            "expected TaskContractNotFound, got: {err}"
        );
    }

    #[test]
    fn read_returns_read_failed_for_malformed_json() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("task-contract.json"), b"not json").unwrap();

        let reader = FsTaskContractReader::new(dir.path().to_path_buf());
        let err = reader.read(&track_id("my-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::TaskContractReadFailed { .. }),
            "expected TaskContractReadFailed, got: {err}"
        );
    }

    #[test]
    fn test_read_returns_read_failed_for_oversized_contract_file() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        let file = fs::File::create(track_dir.join("task-contract.json")).unwrap();
        file.set_len(MAX_TASK_CONTRACT_BYTES + 1).unwrap();

        let reader = FsTaskContractReader::new(dir.path().to_path_buf());
        let err = reader.read(&track_id("my-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::TaskContractReadFailed { .. }),
            "expected TaskContractReadFailed, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_returns_read_failed_for_symlinked_contract_file() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        let real = track_dir.join("real-task-contract.json");
        fs::write(&real, SAMPLE_JSON).unwrap();
        std::os::unix::fs::symlink(&real, track_dir.join("task-contract.json")).unwrap();

        let reader = FsTaskContractReader::new(dir.path().to_path_buf());
        let err = reader.read(&track_id("my-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::TaskContractReadFailed { .. }),
            "expected TaskContractReadFailed, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_returns_read_failed_for_symlinked_track_dir() {
        let dir = temp_items_dir();
        let real_track_dir = dir.path().join("real-track");
        fs::create_dir_all(&real_track_dir).unwrap();
        fs::write(real_track_dir.join("task-contract.json"), SAMPLE_JSON).unwrap();
        std::os::unix::fs::symlink(&real_track_dir, dir.path().join("my-track")).unwrap();

        let reader = FsTaskContractReader::new(dir.path().to_path_buf());
        let err = reader.read(&track_id("my-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::TaskContractReadFailed { .. }),
            "expected TaskContractReadFailed, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_returns_read_failed_for_symlinked_items_dir() {
        let dir = temp_items_dir();
        let real_items_dir = dir.path().join("real-items");
        let track_dir = real_items_dir.join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("task-contract.json"), SAMPLE_JSON).unwrap();
        let link_items_dir = dir.path().join("items");
        std::os::unix::fs::symlink(&real_items_dir, &link_items_dir).unwrap();

        let reader = FsTaskContractReader::new(link_items_dir);
        let err = reader.read(&track_id("my-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::TaskContractReadFailed { .. }),
            "expected TaskContractReadFailed, got: {err}"
        );
    }

    #[test]
    fn test_read_returns_read_failed_for_items_dir_outside_current_repo() {
        let dir = tempfile::tempdir().unwrap();
        let reader = FsTaskContractReader::new(dir.path().to_path_buf());
        let err = reader.read(&track_id("my-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::TaskContractReadFailed { .. }),
            "expected TaskContractReadFailed, got: {err}"
        );
    }
}
