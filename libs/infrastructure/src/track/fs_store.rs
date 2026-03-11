//! File-system backed TrackReader + TrackWriter using FileLockManager for exclusive access.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use domain::lock::{AgentId, FileLockManager, LockMode};
use domain::{
    DomainError, RepositoryError, TrackId, TrackMetadata, TrackReadError, TrackReader,
    TrackWriteError, TrackWriter,
};

use super::atomic_write::atomic_write_file;
use super::codec::{self, DocumentMeta};

/// Agent ID prefix used by `FsTrackStore` for lock acquisition.
const STORE_AGENT_PREFIX: &str = "sotp-track-store";

/// Global monotonic counter for generating unique per-call agent IDs.
static AGENT_SEQ: AtomicU64 = AtomicU64::new(0);

/// File-system backed TrackReader + TrackWriter.
/// Uses `FileLockManager` for exclusive access during mutations.
/// Uses `atomic_write_file` for crash-safe persistence.
pub struct FsTrackStore<L: FileLockManager> {
    root: PathBuf,
    lock_manager: Arc<L>,
    lock_timeout: Duration,
}

impl<L: FileLockManager> FsTrackStore<L> {
    /// Creates a new `FsTrackStore`.
    ///
    /// # Arguments
    /// * `root` - Root directory containing track item directories (e.g., `track/items/`).
    /// * `lock_manager` - Shared lock manager for exclusive access.
    /// * `lock_timeout` - Timeout for lock acquisition.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, lock_manager: Arc<L>, lock_timeout: Duration) -> Self {
        Self { root: root.into(), lock_manager, lock_timeout }
    }

    /// Returns the path to `metadata.json` for a given track ID.
    fn metadata_path(&self, id: &TrackId) -> PathBuf {
        self.root.join(id.as_str()).join("metadata.json")
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
    fn write_track(
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

impl<L: FileLockManager> TrackReader for FsTrackStore<L> {
    fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> {
        self.read_track(id).map(|opt| opt.map(|(track, _meta)| track)).map_err(TrackReadError::from)
    }
}

impl<L: FileLockManager> TrackWriter for FsTrackStore<L> {
    fn save(&self, track: &TrackMetadata) -> Result<(), TrackWriteError> {
        let path = self.metadata_path(track.id());

        // Ensure the track directory exists so FilePath::new can canonicalize the parent.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                TrackWriteError::Repository(RepositoryError::Message(format!(
                    "failed to create directory {}: {e}",
                    parent.display()
                )))
            })?;
        }

        // Acquire exclusive lock to prevent concurrent save/update races.
        let lock_path = domain::lock::FilePath::new(&path).map_err(|e| {
            TrackWriteError::Repository(RepositoryError::Message(format!(
                "failed to create lock path for {}: {e}",
                path.display()
            )))
        })?;

        let seq = AGENT_SEQ.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let agent_name = format!("{STORE_AGENT_PREFIX}-{pid}-{seq}");
        let agent = AgentId::new(&agent_name);

        let guard = self
            .lock_manager
            .acquire(&lock_path, LockMode::Exclusive, &agent, pid, Some(self.lock_timeout))
            .map_err(|e| {
                TrackWriteError::Repository(RepositoryError::Message(format!(
                    "failed to acquire lock on {}: {e}",
                    path.display()
                )))
            })?;

        // Read existing meta under lock to preserve created_at, or create new meta.
        let mut meta = match self.read_track(track.id()).map_err(TrackWriteError::from)? {
            Some((_existing, mut meta)) => {
                meta.updated_at = Self::now_iso8601();
                meta
            }
            None => DocumentMeta {
                schema_version: 2,
                created_at: Self::now_iso8601(),
                updated_at: Self::now_iso8601(),
                original_status: None,
            },
        };

        // Clear original_status so encode() recomputes status from the domain
        // model rather than preserving a stale value like "archived".
        meta.original_status = None;

        self.write_track(track, &meta).map_err(TrackWriteError::from)?;

        drop(guard);
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

        let lock_path = domain::lock::FilePath::new(&path).map_err(|e| {
            TrackWriteError::Repository(RepositoryError::Message(format!(
                "failed to create lock path for {}: {e}",
                path.display()
            )))
        })?;

        let seq = AGENT_SEQ.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let agent_name = format!("{STORE_AGENT_PREFIX}-{pid}-{seq}");
        let agent = AgentId::new(&agent_name);

        // Acquire exclusive lock.
        let guard = self
            .lock_manager
            .acquire(&lock_path, LockMode::Exclusive, &agent, pid, Some(self.lock_timeout))
            .map_err(|e| {
                TrackWriteError::Repository(RepositoryError::Message(format!(
                    "failed to acquire lock on {}: {e}",
                    path.display()
                )))
            })?;

        // Read current state under lock.
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

        // Explicitly release lock (RAII drop also works, but explicit is clearer).
        drop(guard);

        Ok(track)
    }
}

/// Resolves the metadata.json path from root and track ID.
/// Exposed for CLI composition (e.g., listing available tracks).
#[must_use]
pub fn metadata_json_path(root: &Path, id: &TrackId) -> PathBuf {
    root.join(id.as_str()).join("metadata.json")
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    use domain::{PlanSection, PlanView, TaskId, TrackId, TrackMetadata, TrackStatus, TrackTask};
    use infrastructure_test_helpers::*;

    fn sample_track(id: &str) -> TrackMetadata {
        let task_id = TaskId::new("T1").unwrap();
        let task = TrackTask::new(task_id.clone(), "Implement feature").unwrap();
        let section = PlanSection::new("S1", "Build", Vec::new(), vec![task_id]).unwrap();
        let plan = PlanView::new(Vec::new(), vec![section]);

        TrackMetadata::new(TrackId::new(id).unwrap(), "Test Track", vec![task], plan, None).unwrap()
    }

    /// Test helpers shared across infrastructure tests.
    mod infrastructure_test_helpers {
        use std::path::Path;
        use std::sync::Arc;
        use std::time::Duration;

        use domain::lock::{
            AgentId, FileGuard, FileLockManager, FilePath, LockEntry, LockError, LockMode,
        };

        use super::FsTrackStore;

        /// No-op lock manager for unit tests that don't need real locking.
        pub struct NoOpLockManager;

        impl FileLockManager for NoOpLockManager {
            fn acquire(
                &self,
                path: &FilePath,
                mode: LockMode,
                agent: &AgentId,
                _pid: u32,
                _timeout: Option<Duration>,
            ) -> Result<FileGuard, LockError> {
                Ok(FileGuard::new(path.clone(), mode, agent.clone(), Box::new(|_, _| {})))
            }

            fn release(&self, _path: &FilePath, _agent: &AgentId) -> Result<(), LockError> {
                Ok(())
            }

            fn query(&self, _path: Option<&FilePath>) -> Result<Vec<LockEntry>, LockError> {
                Ok(Vec::new())
            }

            fn cleanup(&self) -> Result<usize, LockError> {
                Ok(0)
            }

            fn extend(
                &self,
                _path: &FilePath,
                _agent: &AgentId,
                _additional: Duration,
            ) -> Result<(), LockError> {
                Ok(())
            }
        }

        /// Creates a `FsTrackStore` backed by a temporary directory with a no-op lock manager.
        pub fn test_store(root: &Path) -> FsTrackStore<NoOpLockManager> {
            FsTrackStore::new(root, Arc::new(NoOpLockManager), Duration::from_secs(5))
        }
    }

    #[test]
    fn test_save_and_find_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());
        let track = sample_track("test-track");

        store.save(&track).unwrap();
        let loaded = store.find(track.id()).unwrap().unwrap();
        assert_eq!(loaded, track);
    }

    #[test]
    fn test_find_returns_none_for_missing_track() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());
        let id = TrackId::new("nonexistent").unwrap();

        let result = store.find(&id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_mutates_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());
        let track = sample_track("test-track");

        store.save(&track).unwrap();

        let task_id = TaskId::new("T1").unwrap();
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
        let store = test_store(dir.path());
        let id = TrackId::new("nonexistent").unwrap();

        let result = store.update(&id, |_| Ok(()));
        assert!(matches!(
            result,
            Err(TrackWriteError::Repository(RepositoryError::TrackNotFound(_)))
        ));
    }

    #[test]
    fn test_save_preserves_created_at() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());
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
}
