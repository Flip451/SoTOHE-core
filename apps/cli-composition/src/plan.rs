//! `plan` command family — `PlanCompositionRoot` impl methods.

use std::path::PathBuf;
use std::sync::Arc;

use infrastructure::codex_planner::CodexPlannerAdapter;
use usecase::planner::{PlannerInteractor, PlannerPort};

// ---------------------------------------------------------------------------
// Composition root
// ---------------------------------------------------------------------------

/// Composition root for the `plan` command family.
///
/// Wires `CodexPlannerAdapter` (infrastructure) into `PlannerInteractor`
/// (usecase), then injects `PlannerInteractor` into `PlanDriver` (cli_driver)
/// so the bin layer can invoke planning without owning any subprocess logic.
/// The session log directory (`runtime_dir`) is supplied here and forwarded to
/// the adapter at construction time — it does not cross the usecase port boundary.
pub struct PlanCompositionRoot {
    runtime_dir: PathBuf,
}

impl PlanCompositionRoot {
    /// Create a new `PlanCompositionRoot`.
    ///
    /// `runtime_dir` is the directory where the planner adapter writes session log files.
    pub fn new(runtime_dir: PathBuf) -> Self {
        Self { runtime_dir }
    }
}

impl PlanCompositionRoot {
    /// Build a wired [`cli_driver::plan::PlanDriver`] for the plan family.
    ///
    /// Wire chain: `CodexPlannerAdapter` → `PlannerInteractor` → `PlanDriver`.
    pub fn plan_driver(&self) -> cli_driver::plan::PlanDriver {
        use usecase::planner::PlannerService;

        let adapter = Arc::new(CodexPlannerAdapter::new(self.runtime_dir.clone()));
        let interactor = Arc::new(PlannerInteractor::new(adapter as Arc<dyn PlannerPort>));
        cli_driver::plan::PlanDriver::new(interactor as Arc<dyn PlannerService>)
    }
}
