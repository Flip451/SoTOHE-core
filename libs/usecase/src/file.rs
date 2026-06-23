//! File use case port.

use std::path::Path;

/// Error returned by [`FileWritePort`] methods.
#[derive(Debug, thiserror::Error)]
pub enum FilePortError {
    /// The infrastructure layer could not fulfill the request.
    #[error("{0}")]
    Unavailable(String),
}

/// Secondary port for atomic file write operations.
pub trait FileWritePort: Send + Sync {
    /// Atomically write `content` to `path` (tmp + fsync + rename).
    ///
    /// # Errors
    ///
    /// Returns [`FilePortError`] on I/O failure.
    fn write_atomic(&self, path: &Path, content: &[u8]) -> Result<(), FilePortError>;
}
