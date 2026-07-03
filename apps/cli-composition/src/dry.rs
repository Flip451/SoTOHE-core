//! `dry` command family — composition logic for the DRY subcommands.
//!
//! `sotp dry write` / `sotp dry results` / `sotp dry check-approved` are
//! backed by their own IN-14 driver services
//! ([`usecase::dry_write_driver`], [`usecase::dry_results_driver`],
//! [`usecase::dry_check_approved_driver`]), wired directly against
//! `infrastructure` adapters in [`DryCompositionRoot::dry_driver`] below.
//! `sotp dry fix-local` remains backed by
//! [`infrastructure::dry_check::dry_fix_local::CodexDryFixLocalRunner`]
//! (OS-08, unaffected by the IN-14 driver split).

use std::sync::Arc;

mod shim;

pub use shim::DryCompositionRoot;

impl DryCompositionRoot {
    /// Construct a wired [`cli_driver::dry::DryDriver`] for injection into the CLI.
    ///
    /// `dry_driver()` only constructs and injects (ADR 2026-06-21-1328 D2); it
    /// never invokes any of the four services itself.
    ///
    /// `service` backs only `dry fix-local` (OS-08). The remaining dry
    /// commands use the new IN-14 driver services wired directly against
    /// `infrastructure` adapters.
    pub fn dry_driver(&self) -> cli_driver::dry::DryDriver {
        let port: Arc<dyn usecase::dry_driver::DryDriverPort> =
            Arc::new(infrastructure::dry_check::DryDriverAdapter::new());
        let service: Arc<dyn usecase::dry_driver::DryDriverService> =
            Arc::new(usecase::dry_driver::DryDriverInteractor::new(port));

        // ── T025 shared adapters ─────────────────────────────────────────────
        // First three shared across all three new interactors; the diff-base
        // factory is shared by write_service and check_approved_service only
        // (dry_results never resolves a diff base).
        let repo_root_adapter =
            Arc::new(infrastructure::dry_check::dry_driver_shared::FsDryRepoRootAdapter);
        let base_branch_adapter =
            Arc::new(infrastructure::dry_check::dry_driver_shared::FsDryBaseBranchAdapter);
        let storage_factory_adapter =
            Arc::new(infrastructure::dry_check::dry_driver_shared::FsDryCheckStorageFactoryAdapter);
        let diff_base_factory_adapter: Arc<dyn usecase::dry_driver_shared::DryDiffBaseFactoryPort> =
            Arc::new(infrastructure::dry_check::dry_driver_shared::FsDryDiffBaseFactoryAdapter);

        // ── write_service (T026/T027/T028) ───────────────────────────────────
        let write_service = Arc::new(usecase::dry_write_driver::DryWriteDriverInteractor::new(
            repo_root_adapter.clone(),
            base_branch_adapter.clone(),
            diff_base_factory_adapter.clone(),
            Arc::new(infrastructure::dry_check::dry_write_driver::FsDryWriteConfigLoaderAdapter),
            Arc::new(infrastructure::dry_check::dry_write_driver::FsDryCorpusFragmentsAdapter),
            storage_factory_adapter.clone(),
            Arc::new(infrastructure::dry_check::dry_write_driver::DryCheckServiceFactoryAdapter),
            Arc::new(infrastructure::dry_check::dry_write_driver::FsDryCorpusRootManifestAdapter),
        ));

        // ── results_service (T029) ────────────────────────────────────────────
        let results_service =
            Arc::new(usecase::dry_results_driver::DryResultsDriverInteractor::new(
                repo_root_adapter.clone(),
                storage_factory_adapter.clone(),
            ));

        // ── check_approved_service (T030/T031) ────────────────────────────────
        let fragment_pipeline = Arc::new(
            usecase::dry_check::fragment_pipeline::DryFragmentPipelineInteractor::new(
                Arc::new(infrastructure::dry_check::GitDryCheckDiffGetter),
                Arc::new(
                    infrastructure::semantic_dup::fragment_extractor_adapter::CodeFragmentExtractorAdapter::new(),
                ),
            ),
        );
        // Timing measurement + GateEval telemetry emission live in the bin
        // layer (`apps/cli/src/commands/dry.rs::execute_dry_check_approved`),
        // not here and not in `cli_driver::dry::DryDriver`: the interactor
        // stays a pure 7-port application service (usecase-purity forbids a
        // usecase entrypoint from sampling `std::time::Instant` or invoking a
        // telemetry port internally), and `cli_driver::dry::DryDriver` stays
        // a pure invoke+render controller with no side-effecting I/O beyond
        // the service call and outcome rendering (mirroring
        // `execute_verify_with_telemetry`'s bin-level pattern).
        let check_approved_service =
            Arc::new(usecase::dry_check_approved_driver::DryCheckApprovedDriverInteractor::new(
                repo_root_adapter,
                base_branch_adapter,
                diff_base_factory_adapter,
                Arc::new(
                    infrastructure::track::fixpoint_resolve_driver::FsDryCheckConfigLoaderAdapter,
                ),
                Arc::new(infrastructure::dry_check::FsDryCorpusMetaAdapter),
                fragment_pipeline,
                storage_factory_adapter,
            ));

        cli_driver::dry::DryDriver::new(
            service,
            write_service,
            results_service,
            check_approved_service,
        )
    }
}
