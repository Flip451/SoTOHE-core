//! `semantic_dup` command family — CliApp impl methods and input DTOs.
//!
//! Composition root for all four semantic-dup subcommands:
//! - `find_similar`: embed a query fragment and retrieve top-k results.
//! - `dup_index_build`: extract workspace fragments, embed, and insert into LanceDB.
//! - `dup_check`: check diff fragments against the index (soft gate, always exit 0).
//! - `dup_index_measure_quality`: compute embedding quality metrics over workspace.

mod build;
mod check;
mod common;
mod find_similar;
mod measure_quality;

pub use build::DupIndexBuildInput;
pub use check::DupCheckInput;
pub use find_similar::FindSimilarInput;
pub use measure_quality::DupIndexMeasureQualityInput;

// ── Per-context composition root ──────────────────────────────────────────────

/// Composition root for the `semantic_dup` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct SemanticDupCompositionRoot;

impl SemanticDupCompositionRoot {
    /// Create a new `SemanticDupCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SemanticDupCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticDupCompositionRoot {
    /// Construct a wired [`cli_driver::semantic_dup::SemanticDupDriver`] for injection into the CLI.
    ///
    /// Builds `SemanticDupDriverAdapter`, wraps it in `SemanticDupDriverInteractor`,
    /// and returns a `SemanticDupDriver` ready to handle all `semantic_dup` subcommands.
    pub fn semantic_dup_driver(&self) -> cli_driver::semantic_dup::SemanticDupDriver {
        use std::sync::Arc;

        use usecase::semantic_dup_driver::SemanticDupDriverInteractor;

        use crate::semantic_dup_driver_adapter::SemanticDupDriverAdapter;

        let port = Arc::new(SemanticDupDriverAdapter::new());
        let service = Arc::new(SemanticDupDriverInteractor::new(port));
        cli_driver::semantic_dup::SemanticDupDriver::new(service)
    }
}
