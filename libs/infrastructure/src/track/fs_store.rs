//! File-system backed TrackReader + TrackWriter using atomic writes for crash-safe persistence.

use std::path::{Path, PathBuf};

use domain::{
    DomainError, RepositoryError, TrackId, TrackMetadata, TrackReadError, TrackReader,
    TrackWriteError, TrackWriter,
};

use super::atomic_write::atomic_write_file;
use super::codec::{self, DocumentMeta};

/// File-system backed TrackReader + TrackWriter.
/// Uses `atomic_write_file` for crash-safe persistence.
pub struct FsTrackStore {
    root: PathBuf,
}

impl FsTrackStore {
    /// Creates a new `FsTrackStore`.
    ///
    /// # Arguments
    /// * `root` - Root directory containing track item directories (e.g., `track/items/`).
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the path to `metadata.json` for a given track ID.
    fn metadata_path(&self, id: &TrackId) -> PathBuf {
        self.root.join(id.as_ref()).join("metadata.json")
    }

    /// Reads and decodes `metadata.json` for a given track ID.
    fn read_track(
        &self,
        id: &TrackId,
    ) -> Result<Option<(TrackMetadata, DocumentMeta)>, RepositoryError> {
        let path = self.metadata_path(id);
        if !path.exists() {
            return Ok(None);
        }

        let json = std::fs::read_to_string(&path).map_err(|e| {
            RepositoryError::Message(format!("failed to read {}: {e}", path.display()))
        })?;

        let (track, meta) = codec::decode(&json).map_err(|e| {
            RepositoryError::Message(format!("failed to decode {}: {e}", path.display()))
        })?;

        Ok(Some((track, meta)))
    }

    /// Encodes and atomically writes `metadata.json` for a given track.
    pub(crate) fn write_track(
        &self,
        track: &TrackMetadata,
        meta: &DocumentMeta,
    ) -> Result<(), RepositoryError> {
        let path = self.metadata_path(track.id());

        // Ensure the track directory exists.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                RepositoryError::Message(format!(
                    "failed to create directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        let json = codec::encode(track, meta)
            .map_err(|e| RepositoryError::Message(format!("failed to encode: {e}")))?;

        // Append trailing newline for POSIX compatibility.
        let content = format!("{json}\n");

        atomic_write_file(&path, content.as_bytes()).map_err(|e| {
            RepositoryError::Message(format!("failed to write {}: {e}", path.display()))
        })?;

        Ok(())
    }

    /// Returns the current timestamp as an ISO 8601 string.
    fn now_iso8601() -> String {
        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }
}

impl FsTrackStore {
    /// Read-only metadata load returning both domain model and document metadata.
    ///
    /// Unlike `TrackReader::find`, this also returns `DocumentMeta` (schema version,
    /// timestamps, original status) needed by callers that inspect document-level fields.
    ///
    /// # Errors
    /// Returns `TrackReadError` on I/O or decode failure.
    pub fn find_with_meta(
        &self,
        id: &TrackId,
    ) -> Result<Option<(TrackMetadata, DocumentMeta)>, TrackReadError> {
        self.read_track(id).map_err(TrackReadError::from)
    }
}

impl TrackReader for FsTrackStore {
    fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> {
        self.read_track(id).map(|opt| opt.map(|(track, _meta)| track)).map_err(TrackReadError::from)
    }
}

impl TrackWriter for FsTrackStore {
    fn save(&self, track: &TrackMetadata) -> Result<(), TrackWriteError> {
        let path = self.metadata_path(track.id());

        // Ensure the track directory exists.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                TrackWriteError::Repository(RepositoryError::Message(format!(
                    "failed to create directory {}: {e}",
                    parent.display()
                )))
            })?;
        }

        // Read existing meta to preserve created_at, or create new meta.
        let mut meta = match self.read_track(track.id()).map_err(TrackWriteError::from)? {
            Some((existing, mut meta)) => {
                track.validate_descriptions_unchanged(&existing).map_err(DomainError::from)?;
                track.validate_no_tasks_removed(&existing).map_err(DomainError::from)?;
                meta.updated_at = Self::now_iso8601();
                meta
            }
            None => DocumentMeta {
                schema_version: 2,
                created_at: Self::now_iso8601(),
                updated_at: Self::now_iso8601(),
                original_status: None,
                extra: serde_json::Map::new(),
            },
        };

        // Clear original_status so encode() recomputes status from the domain
        // model rather than preserving a stale value like "archived".
        meta.original_status = None;

        self.write_track(track, &meta).map_err(TrackWriteError::from)?;

        Ok(())
    }

    fn update<F>(&self, id: &TrackId, mutate: F) -> Result<TrackMetadata, TrackWriteError>
    where
        F: FnOnce(&mut TrackMetadata) -> Result<(), DomainError>,
    {
        let path = self.metadata_path(id);

        // Early return if the track directory or metadata.json does not exist.
        if !path.exists() {
            return Err(TrackWriteError::Repository(RepositoryError::TrackNotFound(
                id.to_string(),
            )));
        }

        // Read current state.
        let (mut track, mut meta) =
            self.read_track(id).map_err(TrackWriteError::from)?.ok_or_else(|| {
                TrackWriteError::Repository(RepositoryError::TrackNotFound(id.to_string()))
            })?;

        // Apply mutation (domain logic only, no I/O).
        mutate(&mut track).map_err(TrackWriteError::from)?;

        // Update timestamp and clear original_status so encode() recomputes
        // the status from the (possibly mutated) domain model instead of
        // preserving a stale value like "archived".
        meta.updated_at = Self::now_iso8601();
        meta.original_status = None;
        self.write_track(&track, &meta).map_err(TrackWriteError::from)?;

        Ok(track)
    }
}

impl FsTrackStore {
    /// Execute a closure with full control over both the domain model and
    /// infrastructure metadata. Unlike `update`, this gives the caller full
    /// control over `DocumentMeta` (including `updated_at`) and does NOT
    /// auto-set any timestamps — the closure is responsible for setting them.
    ///
    /// The closure receives `(&mut TrackMetadata, &mut DocumentMeta)` and may
    /// perform multiple mutations in a single read-modify-write cycle. After
    /// the closure returns `Ok`, the state is written to disk atomically.
    /// On `Err`, nothing is written.
    ///
    /// Note: this method relies on single-process sequential execution for
    /// correctness. Concurrent callers are not supported — parallel access
    /// will be handled by worktree isolation (Phase 4 SPEC-04).
    ///
    /// # Errors
    /// Returns `TrackWriteError` if the track is not found, the closure returns
    /// an error, or the write fails.
    pub fn with_locked_document<F>(
        &self,
        id: &TrackId,
        f: F,
    ) -> Result<TrackMetadata, TrackWriteError>
    where
        F: FnOnce(&mut TrackMetadata, &mut DocumentMeta) -> Result<(), DomainError>,
    {
        use fs4::fs_std::FileExt;

        let path = self.metadata_path(id);

        // Early return if metadata.json does not exist.
        if !path.exists() {
            return Err(TrackWriteError::Repository(RepositoryError::TrackNotFound(
                id.to_string(),
            )));
        }

        // Acquire an exclusive advisory lock on a sibling `.lock` file.
        // This serializes concurrent `with_locked_document` calls (e.g., parallel auto-record).
        let lock_path = path.with_extension("json.lock");
        let lock_file = std::fs::File::create(&lock_path).map_err(|e| {
            TrackWriteError::Repository(RepositoryError::Message(format!(
                "failed to create lock file {}: {e}",
                lock_path.display()
            )))
        })?;
        lock_file.lock_exclusive().map_err(|e| {
            TrackWriteError::Repository(RepositoryError::Message(format!(
                "failed to acquire exclusive lock on {}: {e}",
                lock_path.display()
            )))
        })?;

        // Read current state (under lock — guaranteed fresh).
        let (mut track, mut meta) =
            self.read_track(id).map_err(TrackWriteError::from)?.ok_or_else(|| {
                TrackWriteError::Repository(RepositoryError::TrackNotFound(id.to_string()))
            })?;

        // Invoke the closure — the caller controls all mutations including timestamps.
        let result = f(&mut track, &mut meta).map_err(TrackWriteError::from);

        if result.is_ok() {
            // Write the final state; `original_status` is cleared so encode()
            // recomputes the status from the domain model.
            meta.original_status = None;
            self.write_track(&track, &meta).map_err(TrackWriteError::from)?;
        }

        // Lock is released when `lock_file` is dropped (end of scope).
        result.map(|()| track)
    }
}

/// Resolves the metadata.json path from root and track ID.
/// Exposed for CLI composition (e.g., listing available tracks).
#[must_use]
pub fn metadata_json_path(root: &Path, id: &TrackId) -> PathBuf {
    root.join(id.as_ref()).join("metadata.json")
}

/// Read-only metadata load directly from disk.
///
/// Reads and decodes `metadata.json` for a given track ID.
/// Use this for read-only paths (e.g., `track resolve`) that only need
/// to inspect metadata without constructing a full `FsTrackStore`.
///
/// # Errors
/// Returns `RepositoryError` on I/O or decode failure.
pub fn read_track_metadata(
    items_dir: &Path,
    id: &TrackId,
) -> Result<(TrackMetadata, DocumentMeta), RepositoryError> {
    let path = items_dir.join(id.as_ref()).join("metadata.json");
    let json = std::fs::read_to_string(&path).map_err(|err| {
        RepositoryError::Message(format!("cannot read {}: {err}", path.display()))
    })?;
    codec::decode(&json)
        .map_err(|err| RepositoryError::Message(format!("cannot parse {}: {err}", path.display())))
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    use domain::{PlanSection, PlanView, TaskId, TrackId, TrackMetadata, TrackStatus, TrackTask};

    fn sample_track(id: &str) -> TrackMetadata {
        let task_id = TaskId::try_new("T1").unwrap();
        let task = TrackTask::new(task_id.clone(), "Implement feature").unwrap();
        let section = PlanSection::new("S1", "Build", Vec::new(), vec![task_id]).unwrap();
        let plan = PlanView::new(Vec::new(), vec![section]);

        TrackMetadata::new(TrackId::try_new(id).unwrap(), "Test Track", vec![task], plan, None)
            .unwrap()
    }

    #[test]
    fn test_save_and_find_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");

        store.save(&track).unwrap();
        let loaded = store.find(track.id()).unwrap().unwrap();
        assert_eq!(loaded, track);
    }

    #[test]
    fn test_find_returns_none_for_missing_track() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let id = TrackId::try_new("nonexistent").unwrap();

        let result = store.find(&id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_mutates_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");

        store.save(&track).unwrap();

        let task_id = TaskId::try_new("T1").unwrap();
        let updated = store
            .update(track.id(), |t| {
                t.transition_task(&task_id, domain::TaskTransition::Start)?;
                Ok(())
            })
            .unwrap();

        assert_eq!(updated.status(), TrackStatus::InProgress);

        // Verify persistence.
        let reloaded = store.find(track.id()).unwrap().unwrap();
        assert_eq!(reloaded.status(), TrackStatus::InProgress);
    }

    #[test]
    fn test_update_returns_error_for_missing_track() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let id = TrackId::try_new("nonexistent").unwrap();

        let result = store.update(&id, |_| Ok(()));
        assert!(matches!(
            result,
            Err(TrackWriteError::Repository(RepositoryError::TrackNotFound(_)))
        ));
    }

    #[test]
    fn test_save_new_track_succeeds_without_validation() {
        // A brand-new track (no previous on disk) should succeed regardless of descriptions.
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("new-track");

        let result = store.save(&track);
        assert!(result.is_ok());
    }

    #[test]
    fn test_save_with_unchanged_descriptions_succeeds() {
        // Re-saving with identical task descriptions should succeed.
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");

        store.save(&track).unwrap();

        // Save again with the exact same data — should succeed.
        let result = store.save(&track);
        assert!(result.is_ok());
    }

    #[test]
    fn test_save_with_mutated_description_returns_error() {
        // Saving with a changed task description on an existing track should fail.
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");

        store.save(&track).unwrap();

        // Build a track with the same ID but a different task description.
        let task_id = TaskId::try_new("T1").unwrap();
        let mutated_task = TrackTask::new(task_id.clone(), "MUTATED description").unwrap();
        let section = PlanSection::new("S1", "Build", Vec::new(), vec![task_id]).unwrap();
        let plan = PlanView::new(Vec::new(), vec![section]);
        let mutated_track = TrackMetadata::new(
            TrackId::try_new("test-track").unwrap(),
            "Test Track",
            vec![mutated_task],
            plan,
            None,
        )
        .unwrap();

        let result = store.save(&mutated_track);
        assert!(matches!(
            result,
            Err(TrackWriteError::Domain(DomainError::Validation(
                domain::ValidationError::TaskDescriptionMutated { .. }
            )))
        ));
    }

    #[test]
    fn test_save_adding_new_task_to_existing_track_succeeds() {
        // Adding a brand-new task ID (not present in the previous version) should succeed.
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");

        store.save(&track).unwrap();

        // Build a new version that keeps T1 unchanged and adds T2.
        let task_id_t1 = TaskId::try_new("T1").unwrap();
        let task_id_t2 = TaskId::try_new("T2").unwrap();
        let task_t1 = TrackTask::new(task_id_t1.clone(), "Implement feature").unwrap();
        let task_t2 = TrackTask::new(task_id_t2.clone(), "New task").unwrap();
        let section =
            PlanSection::new("S1", "Build", Vec::new(), vec![task_id_t1, task_id_t2]).unwrap();
        let plan = PlanView::new(Vec::new(), vec![section]);
        let extended_track = TrackMetadata::new(
            TrackId::try_new("test-track").unwrap(),
            "Test Track",
            vec![task_t1, task_t2],
            plan,
            None,
        )
        .unwrap();

        let result = store.save(&extended_track);
        assert!(result.is_ok());
    }

    #[test]
    fn test_save_with_removed_task_returns_error() {
        // Removing a task that existed in the previous version must fail.
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track"); // has T1

        store.save(&track).unwrap();

        // Build a new version with no tasks (T1 removed).
        let section = PlanSection::new("S1", "Build", Vec::new(), Vec::new()).unwrap();
        let plan = PlanView::new(Vec::new(), vec![section]);
        let empty_track = TrackMetadata::new(
            TrackId::try_new("test-track").unwrap(),
            "Test Track",
            Vec::new(),
            plan,
            None,
        )
        .unwrap();

        let result = store.save(&empty_track);
        assert!(matches!(
            result,
            Err(TrackWriteError::Domain(DomainError::Validation(
                domain::ValidationError::TaskRemoved { .. }
            )))
        ));
    }

    #[test]
    fn test_save_preserves_created_at() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");

        store.save(&track).unwrap();

        // Read the raw JSON to check created_at.
        let path = dir.path().join("test-track").join("metadata.json");
        let json = std::fs::read_to_string(&path).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        let created_at = doc["created_at"].as_str().unwrap().to_owned();

        // Save again — created_at should be preserved.
        store.save(&track).unwrap();

        let json2 = std::fs::read_to_string(&path).unwrap();
        let doc2: serde_json::Value = serde_json::from_str(&json2).unwrap();
        assert_eq!(doc2["created_at"].as_str().unwrap(), created_at);
    }

    // --- with_locked_document tests ---

    #[test]
    fn test_with_locked_document_returns_error_for_missing_track() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let id = TrackId::try_new("nonexistent").unwrap();

        let result = store.with_locked_document(&id, |_, _| Ok(()));
        assert!(matches!(
            result,
            Err(TrackWriteError::Repository(RepositoryError::TrackNotFound(_)))
        ));
    }

    #[test]
    fn test_with_locked_document_mutates_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");
        store.save(&track).unwrap();

        let task_id = TaskId::try_new("T1").unwrap();
        let updated = store
            .with_locked_document(track.id(), |t, _meta| {
                t.transition_task(&task_id, domain::TaskTransition::Start)?;
                Ok(())
            })
            .unwrap();

        assert_eq!(updated.status(), TrackStatus::InProgress);

        // Verify persistence.
        let reloaded = store.find(track.id()).unwrap().unwrap();
        assert_eq!(reloaded.status(), TrackStatus::InProgress);
    }

    #[test]
    fn test_with_locked_document_does_not_auto_set_updated_at() {
        // The closure sets updated_at explicitly; with_locked_document must not
        // override it.
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");
        store.save(&track).unwrap();

        let sentinel = "1999-01-01T00:00:00Z".to_owned();
        store
            .with_locked_document(track.id(), |_t, meta| {
                meta.updated_at = sentinel.clone();
                Ok(())
            })
            .unwrap();

        let path = dir.path().join("test-track").join("metadata.json");
        let json = std::fs::read_to_string(&path).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(doc["updated_at"].as_str().unwrap(), sentinel);
    }

    #[test]
    fn test_with_locked_document_does_not_write_on_closure_error() {
        // If the closure returns Err, nothing should be written to disk.
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");
        store.save(&track).unwrap();

        // Record the updated_at before the failed call.
        let path = dir.path().join("test-track").join("metadata.json");
        let json_before = std::fs::read_to_string(&path).unwrap();

        let result = store.with_locked_document(track.id(), |_, _| {
            Err(DomainError::Validation(domain::ValidationError::InvalidTaskId(
                "intentional error".to_owned(),
            )))
        });
        assert!(result.is_err());

        // File must be unchanged.
        let json_after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(json_before, json_after);
    }
}
