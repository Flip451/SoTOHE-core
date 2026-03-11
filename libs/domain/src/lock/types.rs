use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A canonicalized file path used as lock key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FilePath(PathBuf);

impl FilePath {
    /// Creates a new `FilePath` by canonicalizing the given path.
    ///
    /// If the file does not exist, canonicalizes the parent directory
    /// and appends the file name, so locks can be acquired on files
    /// that will be created by the protected operation.
    ///
    /// # Errors
    /// Returns `LockError::InvalidPath` if neither the path nor its parent
    /// can be canonicalized.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, super::error::LockError> {
        let path = path.as_ref();
        // Try direct canonicalization first (file exists).
        if let Ok(canonical) = std::fs::canonicalize(path) {
            return Ok(Self(canonical));
        }
        // Fall back: canonicalize the parent and append the file name.
        let parent = path.parent().unwrap_or(Path::new("."));
        let file_name = path.file_name().ok_or_else(|| super::error::LockError::InvalidPath {
            path: path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "no file name"),
        })?;
        let canonical_parent = std::fs::canonicalize(parent).map_err(|source| {
            super::error::LockError::InvalidPath { path: path.to_path_buf(), source }
        })?;
        Ok(Self(canonical_parent.join(file_name)))
    }

    /// Creates a `FilePath` from an already-canonicalized path without I/O.
    #[must_use]
    pub fn from_canonical(path: PathBuf) -> Self {
        Self(path)
    }

    /// Returns the inner path reference.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl fmt::Display for FilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.display().fmt(f)
    }
}

/// Identifies the agent holding or requesting a lock.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentId(String);

impl AgentId {
    /// Creates a new `AgentId`.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the inner string reference.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Maps to Rust's borrow semantics:
/// - `Shared` ≈ `&T` — multiple readers allowed
/// - `Exclusive` ≈ `&mut T` — single writer, no concurrent readers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    Shared,
    Exclusive,
}

impl fmt::Display for LockMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shared => f.write_str("shared"),
            Self::Exclusive => f.write_str("exclusive"),
        }
    }
}

/// A single lock entry in the registry.
#[derive(Debug, Clone)]
pub struct LockEntry {
    /// The locked file path.
    pub path: FilePath,
    /// The lock mode (shared or exclusive).
    pub mode: LockMode,
    /// The agent holding the lock.
    pub agent: AgentId,
    /// The process ID of the agent.
    pub pid: u32,
    /// When the lock was acquired.
    pub acquired_at: SystemTime,
    /// When the lock expires.
    pub expires_at: SystemTime,
}
