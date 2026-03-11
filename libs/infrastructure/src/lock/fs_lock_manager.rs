use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use domain::lock::{AgentId, FileGuard, FileLockManager, FilePath, LockEntry, LockError, LockMode};
use serde::{Deserialize, Serialize};

/// Default TTL for lock entries (5 minutes).
const DEFAULT_TTL: Duration = Duration::from_secs(300);

/// Serializable lock mode for registry.json persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SerializableMode {
    Shared,
    Exclusive,
}

impl From<LockMode> for SerializableMode {
    fn from(mode: LockMode) -> Self {
        match mode {
            LockMode::Shared => Self::Shared,
            LockMode::Exclusive => Self::Exclusive,
        }
    }
}

impl From<SerializableMode> for LockMode {
    fn from(mode: SerializableMode) -> Self {
        match mode {
            SerializableMode::Shared => Self::Shared,
            SerializableMode::Exclusive => Self::Exclusive,
        }
    }
}

/// Serializable lock entry for registry.json persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryEntry {
    path: String,
    mode: SerializableMode,
    agent: String,
    pid: u32,
    acquired_at_secs: u64,
    expires_at_secs: u64,
}

impl RegistryEntry {
    fn from_lock_entry(entry: &LockEntry) -> Self {
        Self {
            path: entry.path.as_path().to_string_lossy().into_owned(),
            mode: entry.mode.into(),
            agent: entry.agent.as_str().to_owned(),
            pid: entry.pid,
            acquired_at_secs: entry
                .acquired_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            expires_at_secs: entry
                .expires_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    fn to_lock_entry(&self) -> LockEntry {
        LockEntry {
            path: FilePath::from_canonical(PathBuf::from(&self.path)),
            mode: self.mode.into(),
            agent: AgentId::new(&self.agent),
            pid: self.pid,
            acquired_at: SystemTime::UNIX_EPOCH + Duration::from_secs(self.acquired_at_secs),
            expires_at: SystemTime::UNIX_EPOCH + Duration::from_secs(self.expires_at_secs),
        }
    }
}

/// File-system based implementation of `FileLockManager`.
///
/// Uses `flock` on `.locks/LOCK` for atomic read-modify-write of
/// `.locks/registry.json`. PID-based stale lock detection and TTL
/// expiry provide automatic recovery from crashed agents.
pub struct FsFileLockManager {
    locks_dir: PathBuf,
    default_ttl: Duration,
}

impl FsFileLockManager {
    /// Creates a new `FsFileLockManager` with the given locks directory.
    ///
    /// # Errors
    /// Returns `LockError::RegistryIo` if the directory cannot be created.
    pub fn new(locks_dir: impl AsRef<Path>) -> Result<Self, LockError> {
        let locks_dir = locks_dir.as_ref().to_path_buf();
        fs::create_dir_all(&locks_dir).map_err(LockError::RegistryIo)?;
        Ok(Self { locks_dir, default_ttl: DEFAULT_TTL })
    }

    fn lock_file_path(&self) -> PathBuf {
        self.locks_dir.join("LOCK")
    }

    fn registry_path(&self) -> PathBuf {
        self.locks_dir.join("registry.json")
    }

    fn open_lock_file(&self) -> Result<File, LockError> {
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.lock_file_path())
            .map_err(LockError::RegistryIo)
    }

    fn read_registry(&self) -> Result<Vec<RegistryEntry>, LockError> {
        let path = self.registry_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let data = fs::read_to_string(&path).map_err(LockError::RegistryIo)?;
        if data.trim().is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(&data).map_err(|e| LockError::RegistryIo(std::io::Error::other(e)))
    }

    /// Writes registry atomically: write to temp file, sync, then rename.
    fn write_registry(&self, entries: &[RegistryEntry]) -> Result<(), LockError> {
        use std::io::Write;

        let data = serde_json::to_string_pretty(entries)
            .map_err(|e| LockError::RegistryIo(std::io::Error::other(e)))?;

        let tmp_path = self.locks_dir.join("registry.json.tmp");
        let mut tmp_file = File::create(&tmp_path).map_err(LockError::RegistryIo)?;
        tmp_file.write_all(data.as_bytes()).map_err(LockError::RegistryIo)?;
        tmp_file.sync_all().map_err(LockError::RegistryIo)?;

        fs::rename(&tmp_path, self.registry_path()).map_err(LockError::RegistryIo)
    }

    /// Reap stale entries: dead PIDs and expired timestamps.
    fn reap_stale(entries: &mut Vec<RegistryEntry>) -> usize {
        let now =
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();

        let before = entries.len();
        entries.retain(|entry| {
            if entry.expires_at_secs <= now {
                return false;
            }
            is_pid_alive(entry.pid)
        });
        before - entries.len()
    }

    /// Checks conflicts for a requested lock on a given path.
    fn check_conflicts(
        entries: &[RegistryEntry],
        path_str: &str,
        mode: LockMode,
        agent_str: &str,
    ) -> Result<(), LockError> {
        let existing: Vec<&RegistryEntry> = entries.iter().filter(|e| e.path == path_str).collect();

        if existing.is_empty() {
            return Ok(());
        }

        // Reject same-agent reacquire outright (no idempotent re-acquire, no upgrading).
        // The caller should use `extend` to renew or release first.
        for entry in &existing {
            if entry.agent == agent_str {
                return Err(LockError::AlreadyHeld {
                    path: FilePath::from_canonical(PathBuf::from(path_str)),
                    agent: AgentId::new(agent_str),
                });
            }
        }

        match mode {
            LockMode::Exclusive => {
                // Any existing lock (by another agent) blocks exclusive.
                for entry in &existing {
                    if entry.agent != agent_str {
                        if entry.mode == SerializableMode::Exclusive {
                            return Err(LockError::ExclusivelyHeld {
                                holder: AgentId::new(&entry.agent),
                                pid: entry.pid,
                            });
                        }
                        return Err(LockError::SharedLockConflict { count: existing.len() });
                    }
                }
            }
            LockMode::Shared => {
                // Shared is blocked only by an exclusive lock held by another agent.
                for entry in &existing {
                    if entry.agent != agent_str && entry.mode == SerializableMode::Exclusive {
                        return Err(LockError::ExclusivelyHeld {
                            holder: AgentId::new(&entry.agent),
                            pid: entry.pid,
                        });
                    }
                }
            }
        }

        Ok(())
    }
}

impl FileLockManager for FsFileLockManager {
    fn acquire(
        &self,
        path: &FilePath,
        mode: LockMode,
        agent: &AgentId,
        pid: u32,
        timeout: Option<Duration>,
    ) -> Result<FileGuard, LockError> {
        let deadline = timeout.map(|t| Instant::now() + t);
        let path_str = path.as_path().to_string_lossy().into_owned();
        let agent_str = agent.as_str().to_owned();

        loop {
            let lock_file = self.open_lock_file()?;
            let mut flock = fd_lock::RwLock::new(lock_file);
            let _guard = flock.write().map_err(LockError::RegistryIo)?;

            let mut entries = self.read_registry()?;
            let reaped = Self::reap_stale(&mut entries);

            match Self::check_conflicts(&entries, &path_str, mode, &agent_str) {
                Ok(()) => {
                    let now = SystemTime::now();
                    let entry = LockEntry {
                        path: path.clone(),
                        mode,
                        agent: agent.clone(),
                        pid,
                        acquired_at: now,
                        expires_at: now + self.default_ttl,
                    };
                    entries.push(RegistryEntry::from_lock_entry(&entry));
                    self.write_registry(&entries)?;

                    // Build release callback that removes entry from registry.
                    let locks_dir = self.locks_dir.clone();
                    let release_fn = Box::new(move |p: &FilePath, a: &AgentId| {
                        let manager = FsFileLockManager { locks_dir, default_ttl: DEFAULT_TTL };
                        let _ = manager.release(p, a);
                    });

                    return Ok(FileGuard::new(path.clone(), mode, agent.clone(), release_fn));
                }
                Err(conflict) => {
                    // Persist reap results before releasing flock so expired
                    // rows do not linger in registry.json on conflict paths.
                    if reaped > 0 {
                        self.write_registry(&entries)?;
                    }
                    drop(_guard);

                    // AlreadyHeld is a logic error (same agent), not a
                    // transient contention — return immediately, never retry.
                    if matches!(conflict, LockError::AlreadyHeld { .. }) {
                        return Err(conflict);
                    }

                    if let Some(dl) = deadline {
                        if Instant::now() >= dl {
                            let elapsed = timeout.unwrap_or_default().as_millis() as u64;
                            return Err(LockError::Timeout { elapsed_ms: elapsed });
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    } else {
                        return Err(conflict);
                    }
                }
            }
        }
    }

    fn release(&self, path: &FilePath, agent: &AgentId) -> Result<(), LockError> {
        let lock_file = self.open_lock_file()?;
        let mut flock = fd_lock::RwLock::new(lock_file);
        let _guard = flock.write().map_err(LockError::RegistryIo)?;

        let mut entries = self.read_registry()?;
        let reaped = Self::reap_stale(&mut entries);
        let path_str = path.as_path().to_string_lossy();
        let agent_str = agent.as_str();

        let before = entries.len();
        entries.retain(|e| !(e.path == *path_str && e.agent == agent_str));

        if entries.len() == before {
            // Persist reap results even on the NotFound path so expired
            // rows do not linger in registry.json.
            if reaped > 0 {
                self.write_registry(&entries)?;
            }
            return Err(LockError::NotFound { path: path.clone(), agent: agent.clone() });
        }

        self.write_registry(&entries)
    }

    fn query(&self, path: Option<&FilePath>) -> Result<Vec<LockEntry>, LockError> {
        let lock_file = self.open_lock_file()?;
        let mut flock = fd_lock::RwLock::new(lock_file);
        let _guard = flock.write().map_err(LockError::RegistryIo)?;

        let mut entries = self.read_registry()?;
        let reaped = Self::reap_stale(&mut entries);
        if reaped > 0 {
            self.write_registry(&entries)?;
        }

        let result: Vec<LockEntry> = entries
            .iter()
            .filter(|e| path.is_none_or(|p| e.path == p.as_path().to_string_lossy().as_ref()))
            .map(|e| e.to_lock_entry())
            .collect();

        Ok(result)
    }

    fn cleanup(&self) -> Result<usize, LockError> {
        let lock_file = self.open_lock_file()?;
        let mut flock = fd_lock::RwLock::new(lock_file);
        let _guard = flock.write().map_err(LockError::RegistryIo)?;

        let mut entries = self.read_registry()?;
        let reaped = Self::reap_stale(&mut entries);
        if reaped > 0 {
            self.write_registry(&entries)?;
        }
        Ok(reaped)
    }

    fn extend(
        &self,
        path: &FilePath,
        agent: &AgentId,
        additional: Duration,
    ) -> Result<(), LockError> {
        let lock_file = self.open_lock_file()?;
        let mut flock = fd_lock::RwLock::new(lock_file);
        let _guard = flock.write().map_err(LockError::RegistryIo)?;

        let mut entries = self.read_registry()?;
        let reaped = Self::reap_stale(&mut entries);
        let path_str = path.as_path().to_string_lossy();
        let agent_str = agent.as_str();

        let entry = match entries.iter_mut().find(|e| e.path == *path_str && e.agent == agent_str) {
            Some(e) => e,
            None => {
                // Persist reap results even on the NotFound path so expired
                // rows do not linger in registry.json.
                if reaped > 0 {
                    self.write_registry(&entries)?;
                }
                return Err(LockError::NotFound { path: path.clone(), agent: agent.clone() });
            }
        };

        // Round up to avoid silent no-op for sub-second durations (e.g. 500ms → 1s).
        let extra_secs = additional.as_secs() + u64::from(additional.subsec_nanos() > 0);
        entry.expires_at_secs += extra_secs;
        self.write_registry(&entries)
    }
}

/// Checks if a process with the given PID is alive.
fn is_pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use domain::lock::{AgentId, FileLockManager, FilePath, LockError, LockMode};
    use tempfile::TempDir;

    use super::FsFileLockManager;

    fn setup() -> (TempDir, TempDir, FsFileLockManager) {
        let locks_dir = TempDir::new().unwrap();
        let files_dir = TempDir::new().unwrap();
        let manager = FsFileLockManager::new(locks_dir.path()).unwrap();
        (locks_dir, files_dir, manager)
    }

    fn create_test_file(dir: &TempDir, name: &str) -> FilePath {
        let path = dir.path().join(name);
        std::fs::write(&path, "test").unwrap();
        FilePath::new(&path).unwrap()
    }

    #[test]
    fn acquire_and_release_exclusive_lock() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent = AgentId::new("agent-1");
        let pid = std::process::id();

        let guard = manager.acquire(&path, LockMode::Exclusive, &agent, pid, None).unwrap();
        assert_eq!(guard.mode(), LockMode::Exclusive);

        let entries = manager.query(None).unwrap();
        assert_eq!(entries.len(), 1);

        manager.release(&path, &agent).unwrap();

        let entries = manager.query(None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn multiple_shared_locks_coexist() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent1 = AgentId::new("agent-1");
        let agent2 = AgentId::new("agent-2");
        let pid = std::process::id();

        let _g1 = manager.acquire(&path, LockMode::Shared, &agent1, pid, None).unwrap();
        let _g2 = manager.acquire(&path, LockMode::Shared, &agent2, pid, None).unwrap();

        let entries = manager.query(Some(&path)).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn exclusive_lock_blocks_concurrent_exclusive() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent1 = AgentId::new("agent-1");
        let agent2 = AgentId::new("agent-2");
        let pid = std::process::id();

        let _g1 = manager.acquire(&path, LockMode::Exclusive, &agent1, pid, None).unwrap();
        let result = manager.acquire(&path, LockMode::Exclusive, &agent2, pid, None);

        assert!(matches!(result, Err(LockError::ExclusivelyHeld { .. })));
    }

    #[test]
    fn exclusive_lock_blocks_shared_acquire() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent1 = AgentId::new("agent-1");
        let agent2 = AgentId::new("agent-2");
        let pid = std::process::id();

        let _g1 = manager.acquire(&path, LockMode::Exclusive, &agent1, pid, None).unwrap();
        let result = manager.acquire(&path, LockMode::Shared, &agent2, pid, None);

        assert!(matches!(result, Err(LockError::ExclusivelyHeld { .. })));
    }

    #[test]
    fn shared_locks_block_exclusive_acquire() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent1 = AgentId::new("agent-1");
        let agent2 = AgentId::new("agent-2");
        let pid = std::process::id();

        let _g1 = manager.acquire(&path, LockMode::Shared, &agent1, pid, None).unwrap();
        let result = manager.acquire(&path, LockMode::Exclusive, &agent2, pid, None);

        assert!(matches!(result, Err(LockError::SharedLockConflict { .. })));
    }

    #[test]
    fn release_nonexistent_lock_returns_not_found() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent = AgentId::new("agent-1");

        let result = manager.release(&path, &agent);
        assert!(matches!(result, Err(LockError::NotFound { .. })));
    }

    #[test]
    fn extend_increases_expiry() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent = AgentId::new("agent-1");
        let pid = std::process::id();

        let _guard = manager.acquire(&path, LockMode::Shared, &agent, pid, None).unwrap();

        let before = manager.query(Some(&path)).unwrap();
        let expires_before = before[0].expires_at;

        manager.extend(&path, &agent, Duration::from_secs(60)).unwrap();

        let after = manager.query(Some(&path)).unwrap();
        assert!(after[0].expires_at > expires_before);
    }

    #[test]
    fn extend_sub_second_rounds_up_to_one_second() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent = AgentId::new("agent-1");
        let pid = std::process::id();

        let _guard = manager.acquire(&path, LockMode::Shared, &agent, pid, None).unwrap();

        let before = manager.query(Some(&path)).unwrap();
        let expires_before = before[0].expires_at;

        // 500ms should round up to 1 second, not be a no-op.
        manager.extend(&path, &agent, Duration::from_millis(500)).unwrap();

        let after = manager.query(Some(&path)).unwrap();
        assert!(after[0].expires_at > expires_before);
    }

    #[test]
    fn cleanup_removes_expired_entries() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent = AgentId::new("agent-1");
        let pid = std::process::id();

        // Acquire and then manually expire the entry.
        let _guard = manager.acquire(&path, LockMode::Shared, &agent, pid, None).unwrap();

        // Overwrite registry with an expired entry.
        let expired = super::RegistryEntry {
            path: path.as_path().to_string_lossy().into_owned(),
            mode: super::SerializableMode::Shared,
            agent: "agent-1".to_owned(),
            pid,
            acquired_at_secs: 0,
            expires_at_secs: 0, // epoch = expired
        };
        manager.write_registry(&[expired]).unwrap();

        let reaped = manager.cleanup().unwrap();
        assert_eq!(reaped, 1);

        let entries = manager.query(None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn timeout_returns_error_on_conflict() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent1 = AgentId::new("agent-1");
        let agent2 = AgentId::new("agent-2");
        let pid = std::process::id();

        let _g1 = manager.acquire(&path, LockMode::Exclusive, &agent1, pid, None).unwrap();
        let result = manager.acquire(
            &path,
            LockMode::Exclusive,
            &agent2,
            pid,
            Some(Duration::from_millis(100)),
        );

        assert!(matches!(result, Err(LockError::Timeout { .. })));
    }

    #[test]
    fn same_agent_reacquire_is_rejected() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent = AgentId::new("agent-1");
        let pid = std::process::id();

        let _g1 = manager.acquire(&path, LockMode::Exclusive, &agent, pid, None).unwrap();
        // Same agent, same mode — rejected (use extend instead).
        let result = manager.acquire(&path, LockMode::Exclusive, &agent, pid, None);

        assert!(matches!(result, Err(LockError::AlreadyHeld { .. })));
    }

    #[test]
    fn same_agent_mode_change_is_rejected() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent = AgentId::new("agent-1");
        let pid = std::process::id();

        let _g1 = manager.acquire(&path, LockMode::Shared, &agent, pid, None).unwrap();
        // Same agent, different mode — also rejected (no upgrading).
        let result = manager.acquire(&path, LockMode::Exclusive, &agent, pid, None);

        assert!(matches!(result, Err(LockError::AlreadyHeld { .. })));
    }

    #[test]
    fn lock_on_nonexistent_file_uses_parent_canonicalization() {
        let (_locks, files, _manager) = setup();
        // File does not exist, but parent directory does.
        let nonexistent = files.path().join("does-not-exist.rs");
        let file_path = FilePath::new(&nonexistent).unwrap();

        assert!(file_path.as_path().ends_with("does-not-exist.rs"));
        assert!(file_path.as_path().is_absolute());
    }

    #[test]
    fn same_agent_reacquire_with_timeout_returns_already_held_immediately() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent = AgentId::new("agent-1");
        let pid = std::process::id();

        let _g1 = manager.acquire(&path, LockMode::Exclusive, &agent, pid, None).unwrap();

        let start = std::time::Instant::now();
        let result = manager.acquire(
            &path,
            LockMode::Exclusive,
            &agent,
            pid,
            Some(Duration::from_millis(500)),
        );

        // Must return AlreadyHeld immediately, not Timeout after 500ms.
        assert!(matches!(result, Err(LockError::AlreadyHeld { .. })));
        assert!(start.elapsed() < Duration::from_millis(200));
    }

    #[test]
    fn invalid_registry_mode_is_rejected() {
        let (_locks, _files, manager) = setup();

        // Write raw JSON with an invalid mode.
        let bad_json = r#"[{"path":"/tmp/test","mode":"unknown","agent":"a","pid":1,"acquired_at_secs":0,"expires_at_secs":9999999999}]"#;
        std::fs::write(manager.registry_path(), bad_json).unwrap();

        // read_registry should fail because "unknown" is not a valid SerializableMode.
        let result = manager.read_registry();
        assert!(result.is_err());
    }

    #[test]
    fn extend_on_expired_entry_returns_not_found() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent = AgentId::new("agent-1");
        let pid = std::process::id();

        // Write an already-expired entry directly into the registry.
        let expired_json = format!(
            r#"[{{"path":"{}","mode":"exclusive","agent":"agent-1","pid":{},"acquired_at_secs":0,"expires_at_secs":1}}]"#,
            path.as_path().to_string_lossy(),
            pid,
        );
        std::fs::write(manager.registry_path(), expired_json).unwrap();

        // extend should reap the expired entry first, then return NotFound.
        let result = manager.extend(&path, &agent, Duration::from_secs(60));
        assert!(matches!(result, Err(LockError::NotFound { .. })));

        // Verify the expired row was actually purged from registry.json.
        let on_disk = std::fs::read_to_string(manager.registry_path()).unwrap();
        assert_eq!(on_disk.trim(), "[]");
    }

    #[test]
    fn query_filters_out_expired_entries() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");

        // Write an already-expired entry directly into the registry.
        let expired_json = format!(
            r#"[{{"path":"{}","mode":"exclusive","agent":"agent-1","pid":{},"acquired_at_secs":0,"expires_at_secs":1}}]"#,
            path.as_path().to_string_lossy(),
            std::process::id(),
        );
        std::fs::write(manager.registry_path(), expired_json).unwrap();

        // query should reap stale entries and return empty.
        let entries = manager.query(Some(&path)).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn release_expired_entry_returns_not_found() {
        let (_locks, files, manager) = setup();
        let path = create_test_file(&files, "test.rs");
        let agent = AgentId::new("agent-1");

        // Write an already-expired entry directly into the registry.
        let expired_json = format!(
            r#"[{{"path":"{}","mode":"exclusive","agent":"agent-1","pid":{},"acquired_at_secs":0,"expires_at_secs":1}}]"#,
            path.as_path().to_string_lossy(),
            std::process::id(),
        );
        std::fs::write(manager.registry_path(), expired_json).unwrap();

        // release should reap the expired entry first, then return NotFound.
        let result = manager.release(&path, &agent);
        assert!(matches!(result, Err(LockError::NotFound { .. })));

        // Verify the expired row was actually purged from registry.json.
        let on_disk = std::fs::read_to_string(manager.registry_path()).unwrap();
        assert_eq!(on_disk.trim(), "[]");
    }
}
