use super::types::{AgentId, FilePath, LockMode};

/// Boxed release callback type for `FileGuard`.
pub type ReleaseFn = Box<dyn FnOnce(&FilePath, &AgentId) + Send>;

/// RAII guard that releases the lock on drop.
///
/// Holds a boxed release callback so the domain layer
/// does not depend on the infrastructure implementation.
pub struct FileGuard {
    path: FilePath,
    mode: LockMode,
    agent: AgentId,
    release_fn: Option<ReleaseFn>,
}

impl FileGuard {
    /// Creates a new `FileGuard`.
    #[must_use]
    pub fn new(path: FilePath, mode: LockMode, agent: AgentId, release_fn: ReleaseFn) -> Self {
        Self { path, mode, agent, release_fn: Some(release_fn) }
    }

    /// Returns the locked file path.
    #[must_use]
    pub fn path(&self) -> &FilePath {
        &self.path
    }

    /// Returns the lock mode.
    #[must_use]
    pub fn mode(&self) -> LockMode {
        self.mode
    }

    /// Returns the agent holding this lock.
    #[must_use]
    pub fn agent(&self) -> &AgentId {
        &self.agent
    }
}

impl Drop for FileGuard {
    fn drop(&mut self) {
        if let Some(f) = self.release_fn.take() {
            f(&self.path, &self.agent);
        }
    }
}
