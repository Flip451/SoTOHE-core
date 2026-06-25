//! File use case port.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

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

/// Application-level contract for atomic file write operations.
///
/// `PrimaryAdapter` (`FileDriver`) depends on this interface rather than directly on
/// `FileWritePort` (DIP). `FileInteractor` implements this service by delegating to the
/// injected `FileWritePort`.
pub trait FileService: Send + Sync {
    /// Atomically write content to path (tmp + fsync + rename).
    fn write_atomic(&self, path: PathBuf, content: Vec<u8>) -> Result<(), FilePortError>;
}

/// Interactor that implements `FileService` by delegating to the injected `FileWritePort`.
pub struct FileInteractor {
    port: Arc<dyn FileWritePort>,
}

impl FileInteractor {
    /// Create a new `FileInteractor` wrapping the given `FileWritePort`.
    pub fn new(port: Arc<dyn FileWritePort>) -> Self {
        Self { port }
    }
}

impl FileService for FileInteractor {
    fn write_atomic(&self, path: PathBuf, content: Vec<u8>) -> Result<(), FilePortError> {
        self.port.write_atomic(path.as_path(), &content)
    }
}
