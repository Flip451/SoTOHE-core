//! Request-scoped composition root for the ADR baseline command family.

use std::sync::Arc;

use cli_driver::adr_baseline::AdrBaselineDriver;

/// ADR baseline composition root; request items_dir selects request-scoped adapter wiring.
pub struct AdrBaselineCompositionRoot {}

impl Default for AdrBaselineCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl AdrBaselineCompositionRoot {
    /// Creates an ADR baseline composition root.
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }

    pub fn adr_baseline_driver(&self, project_root: std::path::PathBuf) -> AdrBaselineDriver {
        use infrastructure::adr_baseline::{
            FsAdrBaselineStore, FsGitAdrBaselineSource, SystemClockAdapter,
        };
        use usecase::adr_baseline::{
            AdrBaselineInteractor, AdrBaselineQueryInteractor, AdrBaselineSourcePort,
            AdrBaselineStorePort, AdrBaselineStoreReadPort, ClockPort,
        };

        let store = Arc::new(FsAdrBaselineStore::from(project_root.clone()));
        let source = Arc::new(FsGitAdrBaselineSource::from(project_root));
        let command_service = Arc::new(AdrBaselineInteractor::new(
            store.clone() as Arc<dyn AdrBaselineStorePort>,
            source.clone() as Arc<dyn AdrBaselineSourcePort>,
            Arc::new(SystemClockAdapter) as Arc<dyn ClockPort>,
        ));
        let query_service = Arc::new(AdrBaselineQueryInteractor::new(
            store as Arc<dyn AdrBaselineStoreReadPort>,
            source as Arc<dyn AdrBaselineSourcePort>,
        ));
        AdrBaselineDriver::new(command_service, query_service)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::AdrBaselineCompositionRoot;

    #[test]
    fn test_adr_baseline_driver_wiring_is_available() {
        let project = tempfile::tempdir().unwrap();
        let driver = AdrBaselineCompositionRoot::new().adr_baseline_driver(project.path().into());
        let _ = driver;
    }

    #[test]
    fn adr_baseline_composition_root_is_wiring_only() {
        let source = include_str!("adr_baseline.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap();
        assert!(production_source.contains("-> AdrBaselineDriver"));
        for wired_component in [
            "FsAdrBaselineStore::from(project_root.clone())",
            "FsGitAdrBaselineSource::from(project_root)",
            "SystemClockAdapter",
            "AdrBaselineInteractor::new(",
            "AdrBaselineQueryInteractor::new(",
        ] {
            assert!(
                production_source.contains(wired_component),
                "composition root must wire {wired_component} into the one-way driver path"
            );
        }
        assert!(
            !production_source.contains("timestamp_now()"),
            "composition root must wire, not invoke, the clock adapter"
        );
        assert!(
            !production_source.contains("AdrBaselineRequest"),
            "composition root must neither accept nor transform the compatibility request DTO"
        );
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
    }
}
