//! `task-contract` command family — composition root.
//!
//! [`TaskContractCompositionRoot`] wires filesystem adapters, use-case
//! interactors, and [`TaskContractDriver`] for both the
//! `sotp task-contract check` and `sotp task-contract coverage` subcommands.
//!
//! - `check`: liveness gate (D5) — wires three-port `PreReviewGateInteractor`
//!   (task_contract_reader + signal_reader + impl_plan_reader, D7).
//! - `coverage`: attribution-completeness gate (D5) — wires three-port
//!   `CoverageVerifyInteractor` (task_contract_reader + signal_reader +
//!   impl_plan_reader, D9 task-key referential integrity).

use std::path::PathBuf;
use std::sync::Arc;

use cli_driver::task_contract::TaskContractDriver;
use infrastructure::impl_catalog_signal_reader::FsImplCatalogSignalReader;
use infrastructure::impl_plan_reader::FsImplPlanReader;
use infrastructure::task_contract_reader::FsTaskContractReader;
use usecase::pre_review_gate::{
    CoverageVerifyInteractor, ImplCatalogSignalReaderPort, ImplPlanReaderPort,
    PreReviewGateInteractor, TaskContractReaderPort,
};

/// Composition root for the `task-contract` command family.
///
/// Wires `FsTaskContractReader`, `FsImplCatalogSignalReader`,
/// `FsImplPlanReader`, `PreReviewGateInteractor` (check/liveness, 3-port D7),
/// `CoverageVerifyInteractor` (coverage/attribution-completeness, 3-port D9), and
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
    /// - `FsImplPlanReader` (shared by both services — D7 liveness check
    ///   reads task statuses; D9 coverage check reads task ids)
    ///
    /// Builds `PreReviewGateInteractor` (3-port, liveness check) and
    /// `CoverageVerifyInteractor` (3-port, attribution completeness + task-key
    /// referential integrity), then injects both into `TaskContractDriver::new`.
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
            Arc::clone(&impl_plan_reader),
        ));

        let coverage_service = Arc::new(CoverageVerifyInteractor::new(
            Arc::clone(&task_contract_reader),
            Arc::clone(&signal_reader),
            impl_plan_reader,
        ));

        TaskContractDriver::new(check_service, coverage_service)
    }
}

impl Default for TaskContractCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::TaskContractCompositionRoot;

    #[test]
    fn task_contract_composition_root_is_wiring_only() {
        let source = include_str!("task_contract.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap();
        assert!(production_source.contains("-> TaskContractDriver"));
        assert!(production_source.contains("TaskContractDriver::new("));
        assert!(production_source.contains("FsTaskContractReader::new("));
        assert!(production_source.contains("FsImplCatalogSignalReader::new("));
        assert!(production_source.contains("FsImplPlanReader::new("));
        assert!(production_source.contains("Arc<dyn TaskContractReaderPort>"));
        assert!(production_source.contains("Arc<dyn ImplCatalogSignalReaderPort>"));
        assert!(production_source.contains("Arc<dyn ImplPlanReaderPort>"));
        for wired_component in [
            "Arc::new(FsTaskContractReader::new(items_dir.clone()))",
            "Arc::new(FsImplCatalogSignalReader::new(items_dir.clone()))",
            "Arc::new(FsImplPlanReader::new(items_dir))",
            "PreReviewGateInteractor::new(",
            "CoverageVerifyInteractor::new(",
            "TaskContractDriver::new(check_service, coverage_service)",
        ] {
            assert!(
                production_source.contains(wired_component),
                "composition root must wire {wired_component} into the one-way driver path"
            );
        }
        for forbidden in [
            "CommandOutcome",
            ".handle(",
            "std::fs::",
            "std::process::",
            "std::net::",
            "std::io::",
            "println!",
            "eprintln!",
            "print!",
            "eprint!",
            "ServiceImpl",
        ] {
            assert!(
                !production_source.contains(forbidden),
                "composition root must not contain execution or compatibility path {forbidden}"
            );
        }

        let _root = TaskContractCompositionRoot::new();
    }
}
