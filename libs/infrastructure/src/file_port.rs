//! File port adapter (T023).
//!
//! Implements [`usecase::file::FileWritePort`] by delegating to
//! [`crate::track::atomic_write::atomic_write_file`].

use std::path::Path;

use usecase::file::FileWritePort;

use crate::track::atomic_write::atomic_write_file;

/// Filesystem adapter that implements [`FileWritePort`].
///
/// Delegates to [`atomic_write_file`] for safe, atomic file writes.
pub struct FsFileWriteAdapter;

impl FsFileWriteAdapter {
    /// Create a new `FsFileWriteAdapter`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsFileWriteAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWritePort for FsFileWriteAdapter {
    fn write_atomic(
        &self,
        path: &Path,
        content: &[u8],
    ) -> Result<(), usecase::file::FilePortError> {
        atomic_write_file(path, content)
            .map_err(|e| usecase::file::FilePortError::Unavailable(e.to_string()))
    }
}
