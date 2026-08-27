//! `task-contract` command family — composition root.
//!
//! [`TaskContractCompositionRoot`] wires filesystem adapters, use-case
//! interactors, and [`TaskContractDriver`] for both the
//! `sotp task-contract check` and `sotp task-contract coverage` subcommands.
//!
//! - `check`: liveness gate (D5) — wires `PreReviewGateInteractor` with the
//!   task-contract, signal, plan, and catalogue readers (D7).
//! - `coverage`: attribution-completeness gate (D5) — wires
//!   `CoverageVerifyInteractor` with the same readers (D9 task-key referential
//!   integrity).

use std::path::PathBuf;
use std::sync::Arc;

use cli_driver::task_contract::TaskContractDriver;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::TdddLayerBindingsPort;
use infrastructure::impl_catalog_signal_reader::FsImplCatalogSignalReader;
use infrastructure::impl_plan_reader::FsImplPlanReader;
use infrastructure::task_contract_reader::FsTaskContractReader;
use infrastructure::tddd::tddd_catalogue_document_loader::FsCatalogueDocumentLoader;
use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
use usecase::catalogue_document_loader::AttestedCatalogueDocumentLoaderPort;
use usecase::pre_review_gate::{
    CoverageVerifyInteractor, ImplCatalogSignalReaderPort, ImplPlanReaderPort,
    PreReviewGateInteractor, TaskContractReaderPort,
};

/// Composition root for the `task-contract` command family.
///
/// Wires `FsTaskContractReader`, `FsImplCatalogSignalReader`,
/// `FsImplPlanReader`, `FsCatalogueDocumentLoader`, both gate interactors, and
/// `TaskContractDriver` for the `sotp task-contract` subcommands.
pub struct TaskContractCompositionRoot {
    workspace_root: PathBuf,
}

impl TaskContractCompositionRoot {
    /// Create a new `TaskContractCompositionRoot` for a known workspace root.
    #[must_use]
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    /// Construct a fully-wired [`TaskContractDriver`] for the requested `items_dir`.
    ///
    /// Wires the filesystem adapters from `items_dir` and the explicit
    /// `workspace_root` captured when this composition root was built:
    /// - `FsTaskContractReader` (shared by both services)
    /// - `FsImplCatalogSignalReader` (shared by both services)
    /// - `FsImplPlanReader` (shared by both services — D7 liveness check
    ///   reads task statuses; D9 coverage check reads task ids)
    ///
    /// Builds `PreReviewGateInteractor` (liveness check) and
    /// `CoverageVerifyInteractor` (attribution completeness + task-key
    /// referential integrity), then injects both into `TaskContractDriver::new`.
    #[must_use]
    pub fn task_contract_driver(&self, items_dir: PathBuf) -> TaskContractDriver {
        let task_contract_reader: Arc<dyn TaskContractReaderPort> =
            Arc::new(FsTaskContractReader::new(items_dir.clone()));
        let signal_reader: Arc<dyn ImplCatalogSignalReaderPort> =
            Arc::new(FsImplCatalogSignalReader::new(items_dir.clone()));
        let catalogue_loader: Arc<dyn AttestedCatalogueDocumentLoaderPort> =
            Arc::new(FsCatalogueDocumentLoader::new());
        let layer_bindings: Arc<dyn TdddLayerBindingsPort> =
            Arc::new(FsTdddLayerBindingsAdapter::new());
        let impl_plan_reader: Arc<dyn ImplPlanReaderPort> =
            Arc::new(FsImplPlanReader::new(items_dir.clone()));

        let check_service = Arc::new(PreReviewGateInteractor::new(
            Arc::clone(&task_contract_reader),
            Arc::clone(&signal_reader),
            Arc::clone(&impl_plan_reader),
            Arc::clone(&catalogue_loader),
            Arc::clone(&layer_bindings),
            self.workspace_root.clone(),
            items_dir.clone(),
        ));

        let coverage_service = Arc::new(CoverageVerifyInteractor::new(
            Arc::clone(&task_contract_reader),
            Arc::clone(&signal_reader),
            impl_plan_reader,
            catalogue_loader,
            layer_bindings,
            self.workspace_root.clone(),
            items_dir,
        ));

        TaskContractDriver::new(check_service, coverage_service)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use cli_driver::task_contract::TaskContractInput;

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
            "Arc::new(FsImplPlanReader::new(items_dir.clone()))",
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

        let _root = TaskContractCompositionRoot::new(PathBuf::from("/repo"));
    }

    #[test]
    fn custom_items_dir_uses_repository_root_for_architecture_rules() {
        let repository_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
        let source_track =
            repository_root.join("track/items/2026-08-25-post-fq-identity-regression-repair");
        let temp_root = repository_root.join("tmp");
        if !temp_root.is_dir() || !source_track.join("task-contract.json").is_file() {
            return;
        }

        let temp = tempfile::tempdir_in(temp_root).unwrap();
        let items_dir = temp.path().join("custom/track/items");
        let track_dir = items_dir.join("2026-08-25-post-fq-identity-regression-repair");
        fs::create_dir_all(&track_dir).unwrap();
        for file in [
            "task-contract.json",
            "impl-plan.json",
            "domain-type-signals.json",
            "domain-types.json",
            "domain-types-baseline.json",
        ] {
            fs::copy(source_track.join(file), track_dir.join(file)).unwrap();
        }

        let outcome = TaskContractCompositionRoot::new(repository_root)
            .task_contract_driver(items_dir)
            .handle(TaskContractInput::Check {
                layer: Some("domain".to_owned()),
                track_id: "2026-08-25-post-fq-identity-regression-repair".to_owned(),
            });
        let stderr = outcome.stderr.as_deref().unwrap_or_default();
        assert!(
            !stderr.contains("failed to read catalogue for layer 'domain'"),
            "custom items_dir must still resolve architecture-rules.json from the repository root: {stderr}"
        );
    }
}
