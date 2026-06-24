//! `conventions` command family — `ConventionsCompositionRoot` impl methods.

use std::sync::Arc;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `conventions` command family.
///
/// This family has no injectable adapter dependencies; the infrastructure
/// functions are called directly inside each method.
pub struct ConventionsCompositionRoot;

impl ConventionsCompositionRoot {
    /// Create a new `ConventionsCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConventionsCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl ConventionsCompositionRoot {
    /// Build a wired [`cli_driver::conventions::ConventionsDriver`] for the conventions family.
    pub fn conventions_driver(&self) -> cli_driver::conventions::ConventionsDriver {
        use infrastructure::conventions::FsConventionsAdapter;

        let port = Arc::new(FsConventionsAdapter::new());
        cli_driver::conventions::ConventionsDriver::new(port)
    }
}
