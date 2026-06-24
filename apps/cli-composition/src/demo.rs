//! `sotp demo` (or default when no subcommand is given) — `DemoCompositionRoot`.

use std::sync::Arc;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `demo` command.
///
/// This family has no injectable adapter dependencies.
pub struct DemoCompositionRoot;

impl DemoCompositionRoot {
    /// Create a new `DemoCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DemoCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoCompositionRoot {
    /// Build a wired [`cli_driver::demo::DemoDriver`] for the demo family.
    pub fn demo_driver(&self) -> cli_driver::demo::DemoDriver {
        use infrastructure::demo::FsDemoAdapter;
        use usecase::demo::DemoInteractor;

        let port = Arc::new(FsDemoAdapter::new());
        let service = Arc::new(DemoInteractor::new(port));
        cli_driver::demo::DemoDriver::new(service)
    }
}
