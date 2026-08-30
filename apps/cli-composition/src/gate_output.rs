//! Composition root for the shared gate-output path.

use std::path::PathBuf;
use std::sync::Arc;

use cli_driver::gate_output::GateOutputDriver;
use infrastructure::gate_output::{FsGateLogPersistence, ProcessGateRunner};
use usecase::gate_output::{
    GateLogPersistencePort, GateProcessPort, GateRunInteractor, GateRunService,
};

/// Composition root that wires the process and persistence adapters into the
/// gate-output driver.
pub struct GateOutputComposition;

impl GateOutputComposition {
    /// Builds a gate-output driver rooted at `trusted_root`.
    #[must_use]
    pub fn build(trusted_root: PathBuf) -> GateOutputDriver {
        let runner: Arc<dyn GateProcessPort> = Arc::new(ProcessGateRunner::new());
        let logs: Arc<dyn GateLogPersistencePort> =
            Arc::new(FsGateLogPersistence::new(trusted_root));
        let service: Arc<dyn GateRunService> = Arc::new(GateRunInteractor::new(runner, logs));
        GateOutputDriver::new(service)
    }
}

#[cfg(all(test, unix))]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    #[test]
    fn test_gate_output_composition_wires_process_and_persistence() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let driver = GateOutputComposition::build(root.path().to_path_buf());
        let missing_command = root.path().join("missing-gate-command");

        let outcome = driver.invoke(cli_driver::gate_output::GateOutputInput::new(
            "composition".to_owned(),
            vec![OsString::from(missing_command.as_os_str())],
        ));

        assert_eq!(outcome.exit_code, 1);
        let stdout = outcome.stdout.expect("spawn failure should render stdout");
        let log_path = stdout
            .lines()
            .find_map(|line| line.strip_prefix("log: "))
            .map(PathBuf::from)
            .expect("spawn failure should report the persisted log path");
        assert!(log_path.starts_with(root.path().join("tmp/gate")));
    }
}
