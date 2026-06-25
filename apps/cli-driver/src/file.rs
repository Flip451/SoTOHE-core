//! `file` command family — primary adapter driver.
//!
//! `FileDriver` holds an injected [`usecase::file::FileService`] and exposes
//! `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::file::FileService;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `file` command family.
pub enum FileInput {
    /// Atomically write `content` to `path` (tmp + fsync + rename).
    WriteAtomic {
        /// Destination file path.
        path: PathBuf,
        /// Bytes to write.
        content: Vec<u8>,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `file` command family.
///
/// Holds an injected [`FileService`]; exposes `handle(input) -> CommandOutcome`.
pub struct FileDriver {
    service: Arc<dyn FileService>,
}

impl FileDriver {
    /// Create a new `FileDriver` with the given `FileService`.
    pub fn new(service: Arc<dyn FileService>) -> Self {
        Self { service }
    }

    /// Handle a file command.
    pub fn handle(&self, input: FileInput) -> CommandOutcome {
        match input {
            FileInput::WriteAtomic { path, content } => self.file_write_atomic(path, content),
        }
    }

    fn file_write_atomic(&self, path: PathBuf, content: Vec<u8>) -> CommandOutcome {
        match self.service.write_atomic(path, content) {
            Ok(()) => CommandOutcome::success(Some("[OK] file written".to_owned())),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }
}
