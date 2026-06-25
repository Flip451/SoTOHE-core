//! `conventions` command family — `ConventionsCompositionRoot` impl methods.

use std::sync::Arc;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `conventions` command family.
///
/// Wires the `ConventionsPort` adapter into `ConventionsInteractor`, then injects
/// `ConventionsInteractor` into `ConventionsDriver`.
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
    ///
    /// Wire chain: `FsConventionsAdapter` → `ConventionsInteractor` → `ConventionsDriver`.
    pub fn conventions_driver(&self) -> cli_driver::conventions::ConventionsDriver {
        use infrastructure::conventions::FsConventionsAdapter;
        use usecase::conventions::{ConventionsInteractor, ConventionsPort};

        let adapter = Arc::new(FsConventionsAdapter::new());
        let interactor = Arc::new(ConventionsInteractor::new(adapter as Arc<dyn ConventionsPort>));
        cli_driver::conventions::ConventionsDriver::new(interactor)
    }
}
