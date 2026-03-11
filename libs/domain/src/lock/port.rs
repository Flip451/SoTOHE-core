use std::time::Duration;

use super::error::LockError;
use super::guard::FileGuard;
use super::types::{AgentId, FilePath, LockEntry, LockMode};

/// Port for file lock management.
///
/// Implementations must be `Send + Sync` for use across threads
/// within the CLI process.
pub trait FileLockManager: Send + Sync {
    /// Acquires a lock on the given path.
    ///
    /// # Errors
    /// - `LockError::ExclusivelyHeld` if another agent holds an exclusive lock.
    /// - `LockError::SharedLockConflict` if shared locks exist and exclusive is requested.
    /// - `LockError::Timeout` if `timeout` elapses before the lock is available.
    /// - `LockError::RegistryIo` on I/O failure.
    fn acquire(
        &self,
        path: &FilePath,
        mode: LockMode,
        agent: &AgentId,
        pid: u32,
        timeout: Option<Duration>,
    ) -> Result<FileGuard, LockError>;

    /// Explicitly releases a lock. Used by CLI subcommand path
    /// where RAII drop is not practical (Python hook → CLI invoke → process exits).
    ///
    /// # Errors
    /// - `LockError::NotFound` if no matching lock exists.
    /// - `LockError::RegistryIo` on I/O failure.
    fn release(&self, path: &FilePath, agent: &AgentId) -> Result<(), LockError>;

    /// Queries all current locks. If `path` is `Some`, filters to that file.
    ///
    /// # Errors
    /// - `LockError::RegistryIo` on I/O failure.
    fn query(&self, path: Option<&FilePath>) -> Result<Vec<LockEntry>, LockError>;

    /// Removes stale entries (dead PIDs, expired timestamps).
    /// Returns the number of entries reaped.
    ///
    /// # Errors
    /// - `LockError::RegistryIo` on I/O failure.
    fn cleanup(&self) -> Result<usize, LockError>;

    /// Extends the expiry of an existing lock.
    ///
    /// # Errors
    /// - `LockError::NotFound` if no matching lock exists.
    /// - `LockError::RegistryIo` on I/O failure.
    fn extend(
        &self,
        path: &FilePath,
        agent: &AgentId,
        additional: Duration,
    ) -> Result<(), LockError>;
}
