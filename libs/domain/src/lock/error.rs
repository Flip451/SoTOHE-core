use std::path::PathBuf;

/// Errors related to file lock operations.
#[derive(Debug, thiserror::Error)]
pub enum LockError {
    /// The given path cannot be canonicalized.
    #[error("path cannot be canonicalized: {path}")]
    InvalidPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The file is exclusively locked by another agent.
    #[error("file is exclusively locked by agent {holder} (pid {pid})")]
    ExclusivelyHeld { holder: super::types::AgentId, pid: u32 },

    /// The file has shared locks, so an exclusive lock cannot be acquired.
    #[error("file has {count} shared lock(s); cannot acquire exclusive lock")]
    SharedLockConflict { count: usize },

    /// The same agent already holds a lock on this path.
    #[error("lock already held by agent {agent} on {path}")]
    AlreadyHeld { path: super::types::FilePath, agent: super::types::AgentId },

    /// No matching lock entry was found.
    #[error("lock not found for path {path} held by agent {agent}")]
    NotFound { path: super::types::FilePath, agent: super::types::AgentId },

    /// Lock acquisition timed out.
    #[error("lock acquisition timed out after {elapsed_ms}ms")]
    Timeout { elapsed_ms: u64 },

    /// I/O error accessing the lock registry.
    #[error("lock registry I/O error")]
    RegistryIo(#[source] std::io::Error),
}
