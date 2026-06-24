//! `guard` command family — per-context composition root and CliApp shim.

use std::sync::Arc;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `guard` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct GuardCompositionRoot;

impl GuardCompositionRoot {
    /// Create a new `GuardCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for GuardCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl GuardCompositionRoot {
    /// Build a wired [`cli_driver::guard::GuardDriver`] for the guard family.
    pub fn guard_driver(&self) -> cli_driver::guard::GuardDriver {
        use infrastructure::shell::ConchShellParser;
        use usecase::guard::GuardCheckInteractor;

        let parser_port = Arc::new(ConchShellParser);
        let service = Arc::new(GuardCheckInteractor::new(parser_port));
        cli_driver::guard::GuardDriver::new(service)
    }
}
