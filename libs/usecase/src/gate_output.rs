//! Typed use case for running a gate and preserving its complete output.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

/// Opaque adapter diagnostic carried across the application boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateAdapterFailureReason {
    reason: String,
}

impl GateAdapterFailureReason {
    /// Wraps adapter-provided diagnostic text without assigning it application meaning.
    #[must_use]
    pub fn new(reason: String) -> GateAdapterFailureReason {
        GateAdapterFailureReason { reason }
    }
}

impl std::fmt::Display for GateAdapterFailureReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.reason)
    }
}

/// A validated command for one gate execution.
#[derive(Debug)]
pub struct GateRunCommand {
    name: String,
    command: Vec<OsString>,
}

impl GateRunCommand {
    /// Creates a command from its presentation label and OS-native argv.
    ///
    /// # Errors
    ///
    /// Returns [`GateRunCommandError::EmptyCommand`] when no executable is
    /// supplied.
    pub fn try_new(
        name: String,
        command: Vec<OsString>,
    ) -> Result<GateRunCommand, GateRunCommandError> {
        if command.is_empty() {
            return Err(GateRunCommandError::EmptyCommand);
        }
        Ok(GateRunCommand { name, command })
    }

    /// Returns the opaque presentation label.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the validated OS-native argv.
    #[must_use]
    pub fn command(&self) -> &[OsString] {
        &self.command
    }
}

/// Validation failures for [`GateRunCommand`].
#[derive(Debug, Error)]
pub enum GateRunCommandError {
    /// No executable was supplied.
    #[error("gate command is empty")]
    EmptyCommand,
}

/// Value-semantic wrapper for an operating-system child exit status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateExitCode {
    value: i32,
}

impl GateExitCode {
    /// Creates an exit-code value.
    #[must_use]
    pub fn new(value: i32) -> GateExitCode {
        GateExitCode { value }
    }

    /// Reports whether the child exited successfully.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.value == 0
    }

    /// Returns the operating-system exit status.
    #[must_use]
    pub fn value(&self) -> i32 {
        self.value
    }
}

/// Path value returned after the corresponding gate log has been persisted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateLogPath {
    path: PathBuf,
}

impl GateLogPath {
    /// Wraps a path after the persistence port has completed its write.
    #[must_use]
    pub fn from_persisted_path(path: PathBuf) -> GateLogPath {
        GateLogPath { path }
    }

    /// Borrows the persisted log path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

/// Opaque token for one persistence-owned gate-log reservation.
#[derive(Debug, PartialEq, Eq)]
pub struct GateLogReservation {
    path: PathBuf,
}

impl GateLogReservation {
    /// Wraps a path selected and exclusively created by a persistence adapter.
    ///
    /// This constructor is intentionally named for the adapter boundary; callers
    /// should obtain reservations from [`GateLogPersistencePort::reserve`].
    #[doc(hidden)]
    #[must_use]
    pub fn from_reserved_path(path: PathBuf) -> GateLogReservation {
        GateLogReservation { path }
    }

    /// Borrows the reserved log path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

/// Filesystem failures while reserving a contained gate log.
#[derive(Debug, Error)]
pub enum GateLogReservationError {
    /// The requested persistence location is not beneath the trusted root.
    #[error("gate log path is outside the trusted root: {0}")]
    OutsideRoot(PathBuf),
    /// A checked path component is a symbolic link.
    #[error("gate log path contains a symlink component: {0}")]
    SymlinkComponent(PathBuf),
    /// The system clock could not provide a unique log suffix.
    #[error("could not read the current time: {0}")]
    Clock(#[source] std::time::SystemTimeError),
    /// The gate-log directory could not be created.
    #[error("could not create the gate-log directory: {0}")]
    CreateDirectory(GateAdapterFailureReason),
    /// The reserved gate-log file could not be created.
    #[error("could not create the gate-log file: {0}")]
    CreateFile(GateAdapterFailureReason),
    /// The encoded gate-log name cannot fit in a filesystem component.
    #[error("encoded gate-log name is too long: {0}")]
    EncodedNameTooLong(GateAdapterFailureReason),
}

/// Filesystem failures while writing a reserved gate log.
#[derive(Debug, Error)]
pub enum GateLogWriteError {
    /// The requested persistence location is not beneath the trusted root.
    #[error("gate log path is outside the trusted root: {0}")]
    OutsideRoot(PathBuf),
    /// A checked path component is a symbolic link.
    #[error("gate log path contains a symlink component: {0}")]
    SymlinkComponent(PathBuf),
    /// The complete gate output could not be written.
    #[error("could not write the gate log: {0}")]
    Write(GateAdapterFailureReason),
}

/// Failure produced by the child-process secondary port.
#[derive(Debug, Error)]
pub enum GateProcessError {
    /// The operating system rejected the child launch.
    #[error("could not spawn gate command: {0}")]
    Spawn(GateAdapterFailureReason),
}

/// Complete opaque child-process bytes paired with its typed status.
#[derive(Debug)]
pub struct GateProcessOutput {
    /// The child exit status.
    pub exit_code: GateExitCode,
    /// Combined stdout and stderr bytes, retained without interpretation.
    pub output: Vec<u8>,
}

/// Closed application result carrying persisted output information.
#[derive(Debug)]
pub enum GateRunResult {
    /// The child started and returned an exit status.
    ChildExited {
        /// The child exit status.
        exit_code: GateExitCode,
        /// The complete captured child output.
        output: Vec<u8>,
        /// The outcome of persisting the complete captured output.
        log: GateLogWriteOutcome,
    },
    /// The child could not be started, but the failure was logged.
    SpawnFailed {
        /// The process-launch failure.
        error: GateProcessError,
        /// The outcome of persisting the launch diagnostic.
        log: GateLogWriteOutcome,
    },
}

/// Application-operation failures returned before child-process launch.
#[derive(Debug, Error)]
pub enum GateRunError {
    /// The log destination could not be reserved.
    #[error("could not prepare gate log: {0}")]
    Prepare(GateLogReservationError),
}

/// Closed post-execution outcome for gate-log persistence.
#[derive(Debug)]
pub enum GateLogWriteOutcome {
    /// The complete output was written and can be referenced by this path.
    Persisted(GateLogPath),
    /// The child result is available, but its complete output is unavailable.
    Unavailable(GateLogWriteError),
}

/// Persistence boundary for contained, symlink-safe gate-log writes.
pub trait GateLogPersistencePort: Send + Sync {
    /// Reserves a unique destination for one gate-log write.
    ///
    /// # Errors
    ///
    /// Returns [`GateLogReservationError`] when the destination cannot be
    /// prepared.
    fn reserve(
        &self,
        command: &GateRunCommand,
    ) -> Result<GateLogReservation, GateLogReservationError>;

    /// Persists complete child output through a previously reserved destination.
    ///
    /// # Errors
    ///
    /// Returns [`GateLogWriteError`] when the reserved destination cannot be
    /// safely written.
    fn persist(
        &self,
        reservation: GateLogReservation,
        contents: &[u8],
    ) -> Result<GateLogPath, GateLogWriteError>;
}

/// Secondary port for executing one validated gate command.
pub trait GateProcessPort: Send + Sync {
    /// Runs the command and captures its complete output.
    ///
    /// # Errors
    ///
    /// Returns [`GateProcessError`] when the operating system rejects the
    /// child launch.
    fn run(&self, command: &GateRunCommand) -> Result<GateProcessOutput, GateProcessError>;
}

/// Inbound application port for one synchronous gate execution.
pub trait GateRunService: Send + Sync {
    /// Reserves a destination, executes the child, and persists its full output
    /// before returning.
    ///
    /// # Errors
    ///
    /// Returns [`GateRunError::Prepare`] when the destination cannot be
    /// reserved before child-process launch.
    fn execute(&self, command: GateRunCommand) -> Result<GateRunResult, GateRunError>;
}

/// Application orchestrator coordinating process execution and log persistence.
pub struct GateRunInteractor {
    runner: Arc<dyn GateProcessPort>,
    logs: Arc<dyn GateLogPersistencePort>,
}

impl GateRunInteractor {
    /// Creates an interactor from the required process and persistence ports.
    #[must_use]
    pub fn new(
        runner: Arc<dyn GateProcessPort>,
        logs: Arc<dyn GateLogPersistencePort>,
    ) -> GateRunInteractor {
        GateRunInteractor { runner, logs }
    }
}

impl GateRunService for GateRunInteractor {
    fn execute(&self, command: GateRunCommand) -> Result<GateRunResult, GateRunError> {
        let reservation = self.logs.reserve(&command).map_err(GateRunError::Prepare)?;

        match self.runner.run(&command) {
            Ok(process_output) => {
                let log = match self.logs.persist(reservation, &process_output.output) {
                    Ok(path) => GateLogWriteOutcome::Persisted(path),
                    Err(error) => GateLogWriteOutcome::Unavailable(error),
                };
                Ok(GateRunResult::ChildExited {
                    exit_code: process_output.exit_code,
                    output: process_output.output,
                    log,
                })
            }
            Err(error) => {
                let diagnostic = error.to_string();
                let log = match self.logs.persist(reservation, diagnostic.as_bytes()) {
                    Ok(path) => GateLogWriteOutcome::Persisted(path),
                    Err(error) => GateLogWriteOutcome::Unavailable(error),
                };
                Ok(GateRunResult::SpawnFailed { error, log })
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct StubRunner {
        result: Mutex<Option<Result<GateProcessOutput, GateProcessError>>>,
    }

    impl GateProcessPort for StubRunner {
        fn run(&self, _command: &GateRunCommand) -> Result<GateProcessOutput, GateProcessError> {
            self.result
                .lock()
                .expect("runner lock should be available")
                .take()
                .expect("runner called once")
        }
    }

    struct RecordingRunner {
        result: Mutex<Option<Result<GateProcessOutput, GateProcessError>>>,
        seen: Mutex<Option<(String, Vec<OsString>)>>,
    }

    impl GateProcessPort for RecordingRunner {
        fn run(&self, command: &GateRunCommand) -> Result<GateProcessOutput, GateProcessError> {
            *self.seen.lock().expect("command lock should be available") =
                Some((command.name().to_owned(), command.command().to_vec()));
            self.result
                .lock()
                .expect("runner lock should be available")
                .take()
                .expect("runner called once")
        }
    }

    struct StubLogs {
        path: GateLogPath,
        contents: Mutex<Vec<Vec<u8>>>,
    }

    impl GateLogPersistencePort for StubLogs {
        fn reserve(
            &self,
            _command: &GateRunCommand,
        ) -> Result<GateLogReservation, GateLogReservationError> {
            Ok(GateLogReservation::from_reserved_path(self.path.as_path().to_path_buf()))
        }

        fn persist(
            &self,
            _reservation: GateLogReservation,
            contents: &[u8],
        ) -> Result<GateLogPath, GateLogWriteError> {
            self.contents.lock().expect("log lock should be available").push(contents.to_vec());
            Ok(self.path.clone())
        }
    }

    struct FailingLogs;

    impl GateLogPersistencePort for FailingLogs {
        fn reserve(
            &self,
            _command: &GateRunCommand,
        ) -> Result<GateLogReservation, GateLogReservationError> {
            Ok(GateLogReservation::from_reserved_path(PathBuf::from("tmp/gate/failing.log")))
        }

        fn persist(
            &self,
            _reservation: GateLogReservation,
            _contents: &[u8],
        ) -> Result<GateLogPath, GateLogWriteError> {
            Err(GateLogWriteError::Write(GateAdapterFailureReason::new("read-only".to_owned())))
        }
    }

    struct RecordingLogs {
        reserved_path: PathBuf,
        reserve_calls: Mutex<usize>,
        live_reservation: Mutex<Option<PathBuf>>,
        persisted: Mutex<Vec<(PathBuf, Vec<u8>)>>,
    }

    impl GateLogPersistencePort for RecordingLogs {
        fn reserve(
            &self,
            _command: &GateRunCommand,
        ) -> Result<GateLogReservation, GateLogReservationError> {
            *self.reserve_calls.lock().expect("reserve lock should be available") += 1;
            assert!(
                self.live_reservation
                    .lock()
                    .expect("live reservation lock should be available")
                    .replace(self.reserved_path.clone())
                    .is_none()
            );
            Ok(GateLogReservation::from_reserved_path(self.reserved_path.clone()))
        }

        fn persist(
            &self,
            reservation: GateLogReservation,
            contents: &[u8],
        ) -> Result<GateLogPath, GateLogWriteError> {
            let path = reservation.as_path().to_path_buf();
            assert_eq!(
                self.live_reservation
                    .lock()
                    .expect("live reservation lock should be available")
                    .take(),
                Some(path.clone())
            );
            self.persisted
                .lock()
                .expect("log lock should be available")
                .push((path.clone(), contents.to_vec()));
            Ok(GateLogPath::from_persisted_path(path))
        }
    }

    struct FailingReserveLogs;

    impl GateLogPersistencePort for FailingReserveLogs {
        fn reserve(
            &self,
            _command: &GateRunCommand,
        ) -> Result<GateLogReservation, GateLogReservationError> {
            Err(GateLogReservationError::CreateFile(GateAdapterFailureReason::new(
                "read-only".to_owned(),
            )))
        }

        fn persist(
            &self,
            _reservation: GateLogReservation,
            _contents: &[u8],
        ) -> Result<GateLogPath, GateLogWriteError> {
            panic!("persist must not be called after reservation failure")
        }
    }

    fn command() -> GateRunCommand {
        GateRunCommand::try_new("unit-check".to_owned(), vec![OsString::from("unit-check")])
            .expect("test command should be valid")
    }

    #[test]
    fn test_gate_run_command_rejects_empty_argv_and_exposes_validated_values() {
        assert!(matches!(
            GateRunCommand::try_new("empty".to_owned(), Vec::new()),
            Err(GateRunCommandError::EmptyCommand)
        ));

        let command = command();
        assert_eq!(command.name(), "unit-check");
        assert_eq!(command.command(), &[OsString::from("unit-check")]);
    }

    #[test]
    fn test_gate_run_command_only_validates_argv_not_log_name_length() {
        let name = "%".repeat(100);
        let command = GateRunCommand::try_new(name.clone(), vec![OsString::from("unit-check")])
            .expect("command construction must not own filesystem name limits");

        assert_eq!(command.name(), name);
    }

    #[test]
    fn test_gate_run_interactor_returns_prepare_error_without_running_child() {
        let runner = Arc::new(StubRunner {
            result: Mutex::new(Some(Ok(GateProcessOutput {
                exit_code: GateExitCode::new(0),
                output: b"child must not run".to_vec(),
            }))),
        });
        let interactor = GateRunInteractor::new(runner.clone(), Arc::new(FailingReserveLogs));

        let result = interactor.execute(command());

        assert!(matches!(
            result,
            Err(GateRunError::Prepare(GateLogReservationError::CreateFile(_)))
        ));
        assert!(runner.result.lock().expect("runner lock should be available").is_some());
    }

    #[test]
    fn test_gate_run_interactor_passes_reserved_destination_to_persist() {
        let reserved_path = PathBuf::from("tmp/gate/reserved-by-port.log");
        let output = b"output written through reservation".to_vec();
        let logs = Arc::new(RecordingLogs {
            reserved_path: reserved_path.clone(),
            reserve_calls: Mutex::new(0),
            live_reservation: Mutex::new(None),
            persisted: Mutex::new(Vec::new()),
        });
        let runner = Arc::new(StubRunner {
            result: Mutex::new(Some(Ok(GateProcessOutput {
                exit_code: GateExitCode::new(0),
                output: output.clone(),
            }))),
        });
        let interactor = GateRunInteractor::new(runner, logs.clone());

        let result = interactor.execute(command()).expect("execution should succeed");

        match result {
            GateRunResult::ChildExited { log, .. } => match log {
                GateLogWriteOutcome::Persisted(path) => {
                    assert_eq!(path.as_path(), reserved_path.as_path());
                }
                GateLogWriteOutcome::Unavailable(error) => {
                    panic!("reserved log should be persisted: {error:?}");
                }
            },
            GateRunResult::SpawnFailed { error, .. } => {
                panic!("unexpected spawn failure: {error:?}");
            }
        }
        assert_eq!(
            logs.persisted.lock().expect("log lock should be available").as_slice(),
            &[(reserved_path, output)]
        );
    }

    #[test]
    fn test_gate_run_interactor_reserves_once_and_consumes_live_reservation() {
        let reserved_path = PathBuf::from("tmp/gate/single-live-reservation.log");
        let output = b"output must consume the live reservation".to_vec();
        let logs = Arc::new(RecordingLogs {
            reserved_path: reserved_path.clone(),
            reserve_calls: Mutex::new(0),
            live_reservation: Mutex::new(None),
            persisted: Mutex::new(Vec::new()),
        });
        let runner = Arc::new(StubRunner {
            result: Mutex::new(Some(Ok(GateProcessOutput {
                exit_code: GateExitCode::new(0),
                output: output.clone(),
            }))),
        });
        let interactor = GateRunInteractor::new(runner, logs.clone());

        let result = interactor.execute(command()).expect("execution should succeed");

        assert!(matches!(
            result,
            GateRunResult::ChildExited { log: GateLogWriteOutcome::Persisted(_), .. }
        ));
        assert_eq!(*logs.reserve_calls.lock().expect("reserve lock should be available"), 1);
        assert!(
            logs.live_reservation
                .lock()
                .expect("live reservation lock should be available")
                .is_none()
        );
        assert_eq!(
            logs.persisted.lock().expect("log lock should be available").as_slice(),
            &[(reserved_path, output)]
        );
    }

    #[test]
    fn test_gate_run_interactor_persists_child_output_and_returns_child_result() {
        let logs = Arc::new(StubLogs {
            path: GateLogPath::from_persisted_path(PathBuf::from("tmp/gate/unit.log")),
            contents: Mutex::new(Vec::new()),
        });
        let runner = Arc::new(StubRunner {
            result: Mutex::new(Some(Ok(GateProcessOutput {
                exit_code: GateExitCode::new(23),
                output: b"[FAIL] item-one: short reason\nfull diagnostic detail\n".to_vec(),
            }))),
        });
        let interactor = GateRunInteractor::new(runner, logs.clone());

        let result = interactor.execute(command()).expect("execution should succeed");

        match result {
            GateRunResult::ChildExited { exit_code, output, log } => {
                assert_eq!(exit_code.value(), 23);
                assert_eq!(output, b"[FAIL] item-one: short reason\nfull diagnostic detail\n");
                match log {
                    GateLogWriteOutcome::Persisted(log_path) => {
                        assert_eq!(log_path.as_path(), Path::new("tmp/gate/unit.log"));
                    }
                    GateLogWriteOutcome::Unavailable(error) => {
                        panic!("unexpected log write failure: {error:?}");
                    }
                }
            }
            GateRunResult::SpawnFailed { error, .. } => {
                panic!("unexpected spawn failure: {error:?}");
            }
        }
        assert_eq!(
            logs.contents.lock().expect("log lock should be available").as_slice(),
            [b"[FAIL] item-one: short reason\nfull diagnostic detail\n".to_vec()]
        );
    }

    #[test]
    fn test_gate_run_service_applies_summary_log_contract_to_child_command() {
        let logs = Arc::new(StubLogs {
            path: GateLogPath::from_persisted_path(PathBuf::from("tmp/gate/aggregate.log")),
            contents: Mutex::new(Vec::new()),
        });
        let runner = Arc::new(RecordingRunner {
            result: Mutex::new(Some(Ok(GateProcessOutput {
                exit_code: GateExitCode::new(23),
                output: b"[FAIL] aggregate-item: short reason\nfull diagnostic detail\n".to_vec(),
            }))),
            seen: Mutex::new(None),
        });
        let interactor = GateRunInteractor::new(runner.clone(), logs.clone());
        let command = GateRunCommand::try_new(
            "aggregate-check".to_owned(),
            vec![
                OsString::from("/bin/sh"),
                OsString::from("-c"),
                OsString::from("printf '[FAIL] aggregate-item: short reason\\n'; exit 23"),
            ],
        )
        .expect("aggregate child command should be valid");

        let result =
            GateRunService::execute(&interactor, command).expect("execution should succeed");

        match result {
            GateRunResult::ChildExited { exit_code, output, log } => {
                assert_eq!(exit_code, GateExitCode::new(23));
                assert_eq!(
                    output,
                    b"[FAIL] aggregate-item: short reason\nfull diagnostic detail\n"
                );
                match log {
                    GateLogWriteOutcome::Persisted(log_path) => {
                        assert_eq!(log_path.as_path(), Path::new("tmp/gate/aggregate.log"));
                    }
                    GateLogWriteOutcome::Unavailable(error) => {
                        panic!("unexpected log write failure: {error:?}");
                    }
                }
            }
            GateRunResult::SpawnFailed { error, .. } => {
                panic!("unexpected spawn failure: {error:?}");
            }
        }

        assert_eq!(
            runner.seen.lock().expect("command lock should be available").as_ref(),
            Some(&(
                "aggregate-check".to_owned(),
                vec![
                    OsString::from("/bin/sh"),
                    OsString::from("-c"),
                    OsString::from("printf '[FAIL] aggregate-item: short reason\\n'; exit 23"),
                ],
            ))
        );
        assert_eq!(
            logs.contents.lock().expect("log lock should be available").as_slice(),
            [b"[FAIL] aggregate-item: short reason\nfull diagnostic detail\n".to_vec()]
        );
    }

    #[test]
    fn test_gate_run_service_persists_success_output_for_summary_contract() {
        let logs = Arc::new(StubLogs {
            path: GateLogPath::from_persisted_path(PathBuf::from("tmp/gate/success.log")),
            contents: Mutex::new(Vec::new()),
        });
        let runner = Arc::new(StubRunner {
            result: Mutex::new(Some(Ok(GateProcessOutput {
                exit_code: GateExitCode::new(0),
                output: b"[PASS] aggregate-item\ncomplete success output\n".to_vec(),
            }))),
        });
        let interactor = GateRunInteractor::new(runner, logs.clone());

        let result =
            GateRunService::execute(&interactor, command()).expect("execution should succeed");

        match result {
            GateRunResult::ChildExited { exit_code, output, log } => {
                assert_eq!(exit_code, GateExitCode::new(0));
                assert_eq!(output, b"[PASS] aggregate-item\ncomplete success output\n");
                match log {
                    GateLogWriteOutcome::Persisted(log_path) => {
                        assert_eq!(log_path.as_path(), Path::new("tmp/gate/success.log"));
                    }
                    GateLogWriteOutcome::Unavailable(error) => {
                        panic!("unexpected log write failure: {error:?}");
                    }
                }
            }
            GateRunResult::SpawnFailed { error, .. } => {
                panic!("unexpected spawn failure: {error:?}");
            }
        }
        assert_eq!(
            logs.contents.lock().expect("log lock should be available").as_slice(),
            [b"[PASS] aggregate-item\ncomplete success output\n".to_vec()]
        );
    }

    #[test]
    fn test_gate_process_and_persistence_ports_are_used_for_spawn_failures() {
        let logs = Arc::new(StubLogs {
            path: GateLogPath::from_persisted_path(PathBuf::from("tmp/gate/spawn.log")),
            contents: Mutex::new(Vec::new()),
        });
        let runner = Arc::new(StubRunner {
            result: Mutex::new(Some(Err(GateProcessError::Spawn(GateAdapterFailureReason::new(
                "missing".to_owned(),
            ))))),
        });
        let interactor = GateRunInteractor::new(runner, logs.clone());

        let result = interactor.execute(command()).expect("spawn failures are closed results");

        match result {
            GateRunResult::SpawnFailed { error, log } => {
                assert!(error.to_string().contains("missing"));
                match log {
                    GateLogWriteOutcome::Persisted(log_path) => {
                        assert_eq!(log_path.as_path(), Path::new("tmp/gate/spawn.log"));
                    }
                    GateLogWriteOutcome::Unavailable(error) => {
                        panic!("unexpected log write failure: {error:?}");
                    }
                }
            }
            GateRunResult::ChildExited { .. } => panic!("unexpected child result"),
        }
        let contents = logs.contents.lock().expect("log lock should be available");
        assert_eq!(contents.len(), 1);
        assert!(
            String::from_utf8_lossy(contents.first().expect("one log should be recorded"))
                .contains("missing")
        );
    }

    #[test]
    fn test_gate_process_port_returns_complete_output_and_preserves_exit_judgment() {
        let success_runner = StubRunner {
            result: Mutex::new(Some(Ok(GateProcessOutput {
                exit_code: GateExitCode::new(0),
                output: b"[PASS] item-one\ncomplete success output\n".to_vec(),
            }))),
        };
        let success = success_runner.run(&command()).expect("successful run should return output");
        assert_eq!(success.exit_code, GateExitCode::new(0));
        assert_eq!(success.output, b"[PASS] item-one\ncomplete success output\n");

        let failure_runner = StubRunner {
            result: Mutex::new(Some(Ok(GateProcessOutput {
                exit_code: GateExitCode::new(23),
                output: b"[FAIL] item-one: short reason\ncomplete failure output\n".to_vec(),
            }))),
        };
        let failure = failure_runner.run(&command()).expect("failed run should return output");
        assert_eq!(failure.exit_code, GateExitCode::new(23));
        assert_eq!(failure.output, b"[FAIL] item-one: short reason\ncomplete failure output\n");
    }

    #[test]
    fn test_gate_run_interactor_keeps_child_result_when_log_write_is_unavailable() {
        let runner = Arc::new(StubRunner {
            result: Mutex::new(Some(Ok(GateProcessOutput {
                exit_code: GateExitCode::new(23),
                output: b"complete output".to_vec(),
            }))),
        });
        let interactor = GateRunInteractor::new(runner, Arc::new(FailingLogs));

        let result = interactor.execute(command()).expect("child result must be preserved");

        match result {
            GateRunResult::ChildExited { exit_code, output, log } => {
                assert_eq!(exit_code, GateExitCode::new(23));
                assert_eq!(output, b"complete output");
                assert!(matches!(
                    log,
                    GateLogWriteOutcome::Unavailable(GateLogWriteError::Write(_))
                ));
            }
            GateRunResult::SpawnFailed { .. } => panic!("unexpected spawn failure"),
        }

        let runner = Arc::new(StubRunner {
            result: Mutex::new(Some(Err(GateProcessError::Spawn(GateAdapterFailureReason::new(
                "missing child".to_owned(),
            ))))),
        });
        let interactor = GateRunInteractor::new(runner, Arc::new(FailingLogs));

        let result = interactor.execute(command()).expect("spawn result must be preserved");

        match result {
            GateRunResult::SpawnFailed { error, log } => {
                assert!(error.to_string().contains("missing child"));
                assert!(matches!(
                    log,
                    GateLogWriteOutcome::Unavailable(GateLogWriteError::Write(_))
                ));
            }
            GateRunResult::ChildExited { .. } => panic!("unexpected child result"),
        }
    }

    #[test]
    fn test_gate_exit_code_and_log_path_are_value_wrappers() {
        let success = GateExitCode::new(0);
        let failure = GateExitCode::new(7);
        assert!(success.is_success());
        assert!(!failure.is_success());
        assert_eq!(failure.value(), 7);

        let path = GateLogPath::from_persisted_path(PathBuf::from("tmp/gate/result.log"));
        assert_eq!(path.as_path(), Path::new("tmp/gate/result.log"));
    }
}
