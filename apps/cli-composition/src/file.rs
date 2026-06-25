//! `sotp file` subcommands — `FileCompositionRoot`.

use std::sync::Arc;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `file` command family.
///
/// Wires the `FileWritePort` adapter into `FileInteractor`, then injects
/// `FileInteractor` into `FileDriver`.
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
    ///
    /// Wire chain: `FsFileWriteAdapter` → `FileInteractor` → `FileDriver`.
    pub fn file_driver(&self) -> cli_driver::file::FileDriver {
        use infrastructure::file_port::FsFileWriteAdapter;
        use usecase::file::{FileInteractor, FileWritePort};

        let adapter = Arc::new(FsFileWriteAdapter::new());
        let interactor = Arc::new(FileInteractor::new(adapter as Arc<dyn FileWritePort>));
        cli_driver::file::FileDriver::new(interactor)
    }
}
