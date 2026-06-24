//! `sotp file` subcommands — `FileCompositionRoot`.

use std::sync::Arc;

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
}
