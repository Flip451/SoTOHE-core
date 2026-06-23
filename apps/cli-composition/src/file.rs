//! `sotp file` subcommands — `FileCompositionRoot`.

use std::path::PathBuf;
use std::sync::Arc;

use crate::{CommandOutcome, error::CompositionError};

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `file` command family.
///
/// This family has no injectable adapter dependencies; the atomic write
/// function is called directly.
pub struct FileCompositionRoot;

impl FileCompositionRoot {
    /// Create a new `FileCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl FileCompositionRoot {
    /// Build a wired [`cli_driver::file::FileDriver`] for the file family.
    pub fn file_driver(&self) -> cli_driver::file::FileDriver {
        use infrastructure::file_port::FsFileWriteAdapter;

        let port = Arc::new(FsFileWriteAdapter::new());
        cli_driver::file::FileDriver::new(port)
    }

    /// Atomically write `content` to `path` (tmp + fsync + rename).
    ///
    /// # Errors
    ///
    /// Returns `Err` when the atomic write fails (I/O error or path error).
    pub fn file_write_atomic(
        &self,
        path: PathBuf,
        content: &[u8],
    ) -> Result<CommandOutcome, CompositionError> {
        infrastructure::track::atomic_write::atomic_write_file(&path, content)
            .map_err(|e| CompositionError::Infrastructure(format!("atomic write failed: {e}")))?;
        Ok(CommandOutcome::success(None))
    }
}
