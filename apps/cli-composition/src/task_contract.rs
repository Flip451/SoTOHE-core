//! `task-contract` command family — composition root.
//!
//! [`TaskContractCompositionRoot`] wires `FsTaskContractReader`,
//! `FsImplCatalogSignalReader`, `PreReviewGateInteractor`, and
//! `TaskContractDriver` for the `sotp task-contract check` subcommand.

use std::path::PathBuf;
use std::sync::Arc;

use cli_driver::task_contract::{TaskContractDriver, TaskContractInput};
use infrastructure::impl_catalog_signal_reader::FsImplCatalogSignalReader;
use infrastructure::task_contract_reader::FsTaskContractReader;
use usecase::pre_review_gate::PreReviewGateInteractor;

use crate::error::CompositionError;

/// Composition root for the `task-contract` command family.
///
/// Wires `FsTaskContractReader`, `FsImplCatalogSignalReader`,
/// `PreReviewGateInteractor`, and `TaskContractDriver` for the
/// `sotp task-contract check` subcommand.
pub struct TaskContractCompositionRoot;

impl TaskContractCompositionRoot {
    /// Create a new `TaskContractCompositionRoot`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Construct a fully-wired [`TaskContractDriver`] for the requested `items_dir`.
    ///
    /// Wires `FsTaskContractReader` and `FsImplCatalogSignalReader` from
    /// `items_dir` into a `PreReviewGateInteractor` and wraps it as
    /// `Arc<dyn PreReviewGateService>`.
    #[must_use]
    pub fn task_contract_driver(&self, items_dir: PathBuf) -> TaskContractDriver {
        let task_contract_reader = Arc::new(FsTaskContractReader::new(items_dir.clone()));
        let signal_reader = Arc::new(FsImplCatalogSignalReader::new(items_dir));
        let service = Arc::new(PreReviewGateInteractor::new(task_contract_reader, signal_reader));
        TaskContractDriver::new(service)
    }

    /// Wire and invoke the pre-review conformance gate check.
    ///
    /// Constructs a [`TaskContractDriver`] via
    /// [`task_contract_driver(items_dir)`](Self::task_contract_driver), then
    /// dispatches [`TaskContractInput::Check { group, track_id }`]. The driver
    /// parses CLI strings, calls the use case, and renders `Passed`/`Blocked`
    /// outcomes as [`cli_driver::CommandOutcome`].
    ///
    /// # Errors
    ///
    /// Returns [`CompositionError`] if composition fails (currently infallible
    /// for this composition root, but the signature allows future wiring
    /// errors such as config loading).
    pub fn task_contract_check(
        &self,
        group: String,
        track_id: String,
        items_dir: PathBuf,
    ) -> Result<cli_driver::CommandOutcome, CompositionError> {
        let driver = self.task_contract_driver(items_dir);
        Ok(driver.handle(TaskContractInput::Check { group, track_id }))
    }
}

impl Default for TaskContractCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}
