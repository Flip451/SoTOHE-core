//! File-system adapter for review.json persistence.
//!
//! Implements `ReviewJsonReader` and `ReviewJsonWriter` from the domain layer,
//! reading/writing `track/items/<track-id>/review.json` with atomic writes.

use std::path::PathBuf;

use domain::{
    RepositoryError, ReviewJson, ReviewJsonReader, ReviewJsonWriter, TrackId, TrackReadError,
    TrackWriteError,
};

use crate::review_json_codec;
use crate::track::atomic_write::atomic_write_file;

/// File name for the review state file within a track directory.
const REVIEW_JSON_FILENAME: &str = "review.json";

/// File-system store for review.json files.
///
/// `root` is the `track/items/` directory. Review files live at
/// `{root}/{track_id}/review.json`.
pub struct FsReviewJsonStore {
    root: PathBuf,
}

impl FsReviewJsonStore {
    /// Creates a new store rooted at the given items directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the path to review.json for a given track.
    fn review_path(&self, id: &TrackId) -> PathBuf {
        self.root.join(id.as_ref()).join(REVIEW_JSON_FILENAME)
    }
}

/// Checks symlinks on the path and all ancestors below `root` (fail-closed).
///
/// Returns `Ok(false)` if the leaf does not exist, `Ok(true)` if it exists,
/// `Err` if any component is a symlink or cannot be inspected.
fn reject_symlink(path: &std::path::Path, root: &std::path::Path) -> Result<bool, RepositoryError> {
    crate::track::symlink_guard::reject_symlinks_below(path, root)
        .map_err(|e| RepositoryError::Message(e.to_string()))
}

impl ReviewJsonReader for FsReviewJsonStore {
    fn find_review(&self, id: &TrackId) -> Result<Option<ReviewJson>, TrackReadError> {
        let path = self.review_path(id);
        if !reject_symlink(&path, &self.root)? {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path).map_err(|e| {
            RepositoryError::Message(format!("failed to read {}: {e}", path.display()))
        })?;
        let review = review_json_codec::decode(&content).map_err(|e| {
            RepositoryError::Message(format!("failed to decode {}: {e}", path.display()))
        })?;
        Ok(Some(review))
    }
}

impl ReviewJsonWriter for FsReviewJsonStore {
    fn save_review(&self, id: &TrackId, review: &ReviewJson) -> Result<(), TrackWriteError> {
        let path = self.review_path(id);
        // Reject symlinks before any write operation
        if let Err(e) = reject_symlink(&path, &self.root) {
            return Err(TrackWriteError::from(e));
        }
        // Empty review (NoCycle) → remove the file to preserve None == NoCycle contract
        if review.is_empty() {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(e) => {
                    return Err(RepositoryError::Message(format!(
                        "failed to remove {}: {e}",
                        path.display()
                    ))
                    .into());
                }
            }
            // Fsync parent directory to persist the removal durably
            if let Some(parent) = path.parent() {
                let dir = std::fs::File::open(parent).map_err(|e| {
                    RepositoryError::Message(format!(
                        "failed to open dir for fsync {}: {e}",
                        parent.display()
                    ))
                })?;
                dir.sync_all().map_err(|e| {
                    RepositoryError::Message(format!(
                        "failed to fsync dir {}: {e}",
                        parent.display()
                    ))
                })?;
            }
            return Ok(());
        }
        // Ensure the track directory exists and fsync root to persist the new dir entry
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                RepositoryError::Message(format!(
                    "failed to create directory {}: {e}",
                    parent.display()
                ))
            })?;
            // Fsync the root (parent of track dir) to persist the directory entry
            let root_dir = std::fs::File::open(&self.root).map_err(|e| {
                RepositoryError::Message(format!(
                    "failed to open root for fsync {}: {e}",
                    self.root.display()
                ))
            })?;
            root_dir.sync_all().map_err(|e| {
                RepositoryError::Message(format!(
                    "failed to fsync root {}: {e}",
                    self.root.display()
                ))
            })?;
        }
        let json = review_json_codec::encode(review).map_err(|e| {
            RepositoryError::Message(format!("failed to encode {}: {e}", path.display()))
        })?;
        atomic_write_file(&path, json.as_bytes()).map_err(|e| {
            RepositoryError::Message(format!("failed to write {}: {e}", path.display()))
        })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use domain::{
        CycleGroupState, GroupRound, GroupRoundVerdict, ReviewGroupName, ReviewJson, RoundType,
        Timestamp,
    };

    use super::*;

    fn ts(s: &str) -> Timestamp {
        Timestamp::new(s).unwrap()
    }

    fn grn(s: &str) -> ReviewGroupName {
        ReviewGroupName::try_new(s).unwrap()
    }

    fn sample_review() -> ReviewJson {
        let mut groups = BTreeMap::new();
        let mut domain = CycleGroupState::new(vec!["libs/domain/src/lib.rs".into()]);
        domain.record_round(
            GroupRound::success(
                RoundType::Fast,
                ts("2026-03-29T09:48:00Z"),
                "rvw1:sha256:abc",
                GroupRoundVerdict::ZeroFindings,
            )
            .unwrap(),
        );
        groups.insert(grn("domain"), domain);
        groups.insert(grn("other"), CycleGroupState::new(vec!["Makefile.toml".into()]));

        let mut rj = ReviewJson::new();
        rj.start_cycle(
            "2026-03-29T09:47:00Z",
            ts("2026-03-29T09:47:00Z"),
            "main",
            "sha256:policy123",
            groups,
        )
        .unwrap();
        rj
    }

    #[test]
    fn test_find_review_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let store = FsReviewJsonStore::new(dir.path());
        let id = TrackId::try_new("my-track").unwrap();

        let result = store.find_review(&id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_save_and_find_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let store = FsReviewJsonStore::new(dir.path());
        let id = TrackId::try_new("my-track").unwrap();

        let review = sample_review();
        store.save_review(&id, &review).unwrap();

        let loaded = store.find_review(&id).unwrap().unwrap();
        assert_eq!(loaded, review);
    }

    #[test]
    fn test_save_creates_directory_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsReviewJsonStore::new(dir.path());
        let id = TrackId::try_new("new-track").unwrap();

        let review = sample_review();
        store.save_review(&id, &review).unwrap();

        let path = dir.path().join("new-track").join("review.json");
        assert!(path.exists());
    }

    #[test]
    fn test_save_empty_review_does_not_create_file() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let store = FsReviewJsonStore::new(dir.path());
        let id = TrackId::try_new("my-track").unwrap();

        store.save_review(&id, &ReviewJson::new()).unwrap();

        // Empty review should not create a file (NoCycle contract)
        assert!(store.find_review(&id).unwrap().is_none());
    }

    #[test]
    fn test_save_empty_review_removes_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let store = FsReviewJsonStore::new(dir.path());
        let id = TrackId::try_new("my-track").unwrap();

        // Save with data, then save empty → file removed
        store.save_review(&id, &sample_review()).unwrap();
        assert!(store.find_review(&id).unwrap().is_some());

        store.save_review(&id, &ReviewJson::new()).unwrap();
        assert!(store.find_review(&id).unwrap().is_none());
    }

    #[test]
    fn test_save_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let store = FsReviewJsonStore::new(dir.path());
        let id = TrackId::try_new("my-track").unwrap();

        // Save with data, then save different data
        store.save_review(&id, &sample_review()).unwrap();
        let review = sample_review();
        store.save_review(&id, &review).unwrap();

        let loaded = store.find_review(&id).unwrap().unwrap();
        assert_eq!(loaded.cycles().len(), 1);
    }

    #[test]
    fn test_find_review_rejects_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("bad-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("review.json"), "not valid json").unwrap();

        let store = FsReviewJsonStore::new(dir.path());
        let id = TrackId::try_new("bad-track").unwrap();

        let result = store.find_review(&id);
        assert!(result.is_err());
    }
}
