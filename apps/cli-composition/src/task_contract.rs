//! `task-contract` command family — composition root.
//!
//! [`TaskContractCompositionRoot`] wires filesystem adapters, use-case
//! interactors, and [`TaskContractDriver`] for both the
//! `sotp task-contract check` and `sotp task-contract coverage` subcommands.
//!
//! - `check`: liveness gate (D5) — wires three-port `PreReviewGateInteractor`
//!   (task_contract_reader + signal_reader + impl_plan_reader, D7).
//! - `coverage`: attribution-completeness gate (D5) — wires two-port
//!   `CoverageVerifyInteractor` (task_contract_reader + signal_reader).

use std::path::PathBuf;
use std::sync::Arc;

use cli_driver::task_contract::{TaskContractDriver, TaskContractInput};
use infrastructure::impl_catalog_signal_reader::FsImplCatalogSignalReader;
use infrastructure::impl_plan_reader::FsImplPlanReader;
use infrastructure::task_contract_reader::FsTaskContractReader;
use usecase::pre_review_gate::{
    CoverageVerifyInteractor, ImplCatalogSignalReaderPort, ImplPlanReaderPort,
    PreReviewGateInteractor, TaskContractReaderPort,
};

use crate::error::CompositionError;

/// Composition root for the `task-contract` command family.
///
/// Wires `FsTaskContractReader`, `FsImplCatalogSignalReader`,
/// `FsImplPlanReader`, `PreReviewGateInteractor` (check/liveness, 3-port D7),
/// `CoverageVerifyInteractor` (coverage/attribution-completeness, 2-port), and
/// `TaskContractDriver` for the `sotp task-contract` subcommands.
pub struct TaskContractCompositionRoot;

impl TaskContractCompositionRoot {
    /// Create a new `TaskContractCompositionRoot`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Construct a fully-wired [`TaskContractDriver`] for the requested `items_dir`.
    ///
    /// Wires three filesystem adapters from `items_dir`:
    /// - `FsTaskContractReader` (shared by both services)
    /// - `FsImplCatalogSignalReader` (shared by both services)
    /// - `FsImplPlanReader` (used by the liveness check service, D7)
    ///
    /// Builds `PreReviewGateInteractor` (3-port, liveness check) and
    /// `CoverageVerifyInteractor` (2-port, attribution completeness), then
    /// injects both into `TaskContractDriver::new`.
    #[must_use]
    pub fn task_contract_driver(&self, items_dir: PathBuf) -> TaskContractDriver {
        let task_contract_reader: Arc<dyn TaskContractReaderPort> =
            Arc::new(FsTaskContractReader::new(items_dir.clone()));
        let signal_reader: Arc<dyn ImplCatalogSignalReaderPort> =
            Arc::new(FsImplCatalogSignalReader::new(items_dir.clone()));
        let impl_plan_reader: Arc<dyn ImplPlanReaderPort> =
            Arc::new(FsImplPlanReader::new(items_dir));

        let check_service = Arc::new(PreReviewGateInteractor::new(
            Arc::clone(&task_contract_reader),
            Arc::clone(&signal_reader),
            impl_plan_reader,
        ));

        let coverage_service = Arc::new(CoverageVerifyInteractor::new(
            Arc::clone(&task_contract_reader),
            Arc::clone(&signal_reader),
        ));

        TaskContractDriver::new(check_service, coverage_service)
    }

    /// Wire and invoke the pre-review liveness gate check.
    ///
    /// Constructs a [`TaskContractDriver`] via
    /// [`task_contract_driver(items_dir)`](Self::task_contract_driver), then
    /// dispatches [`TaskContractInput::Check { layer, track_id }`]. The driver
    /// parses CLI strings, calls the check use case, and renders `Passed`/`Blocked`
    /// outcomes as [`cli_driver::CommandOutcome`].
    ///
    /// When `layer` is `None`, the gate iterates all 6 canonical TDDD layers
    /// internally and returns a single combined verdict.
    ///
    /// # Errors
    ///
    /// Returns [`CompositionError`] if composition fails (currently infallible
    /// for this composition root, but the signature allows future wiring
    /// errors such as config loading).
    pub fn task_contract_check(
        &self,
        layer: Option<String>,
        track_id: String,
        items_dir: PathBuf,
    ) -> Result<cli_driver::CommandOutcome, CompositionError> {
        let driver = self.task_contract_driver(items_dir);
        Ok(driver.handle(TaskContractInput::Check { layer, track_id }))
    }

    /// Wire and invoke the attribution-completeness coverage check.
    ///
    /// Constructs a [`TaskContractDriver`] via
    /// [`task_contract_driver(items_dir)`](Self::task_contract_driver), then
    /// dispatches [`TaskContractInput::Coverage { track_id }`]. The driver
    /// calls the coverage use case and renders `Passed`/`Blocked` outcomes as
    /// [`cli_driver::CommandOutcome`].
    ///
    /// # Errors
    ///
    /// Returns [`CompositionError`] if composition fails (currently infallible
    /// for this composition root).
    pub fn task_contract_coverage(
        &self,
        track_id: String,
        items_dir: PathBuf,
    ) -> Result<cli_driver::CommandOutcome, CompositionError> {
        let driver = self.task_contract_driver(items_dir);
        Ok(driver.handle(TaskContractInput::Coverage { track_id }))
    }
}

impl Default for TaskContractCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}
