//! `arch` command family — `ArchCompositionRoot` impl methods.

use std::sync::Arc;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `arch` command family.
///
/// This family has no injectable adapter dependencies; adapters are
/// constructed inline inside each method (infrastructure::arch::* functions).
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
    pub fn arch_driver(&self) -> cli_driver::arch::ArchDriver {
        use infrastructure::arch::FsArchAdapter;

        let port = Arc::new(FsArchAdapter::new());
        cli_driver::arch::ArchDriver::new(port)
    }
}
