//! `arch` command family — `ArchCompositionRoot` impl methods.

use std::sync::Arc;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `arch` command family.
///
/// Wires the `ArchPort` adapter into `ArchInteractor`, then injects
/// `ArchInteractor` into `ArchDriver`.
pub struct ArchCompositionRoot;

impl ArchCompositionRoot {
    /// Create a new `ArchCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ArchCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchCompositionRoot {
    /// Build a wired [`cli_driver::arch::ArchDriver`] for the arch family.
    ///
    /// Wire chain: `FsArchAdapter` → `ArchInteractor` → `ArchDriver`.
    pub fn arch_driver(&self) -> cli_driver::arch::ArchDriver {
        use infrastructure::arch::FsArchAdapter;
        use usecase::arch::{ArchInteractor, ArchPort};

        let adapter = Arc::new(FsArchAdapter::new());
        let interactor = Arc::new(ArchInteractor::new(adapter as Arc<dyn ArchPort>));
        cli_driver::arch::ArchDriver::new(interactor)
    }
}
