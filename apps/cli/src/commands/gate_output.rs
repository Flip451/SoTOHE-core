//! CLI parsing and dispatch for `sotp gate-output`.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use cli_composition::GateOutputComposition;
use cli_driver::gate_output::GateOutputInput;

/// Opaque gate label retained for the driver boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GateNameArg {
    value: String,
}

impl GateNameArg {
    /// Creates a gate label from the CLI boundary value.
    #[must_use]
    pub(crate) fn new(value: String) -> GateNameArg {
        GateNameArg { value }
    }

    /// Borrows the gate label.
    #[must_use]
    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }
}

impl FromStr for GateNameArg {
    type Err = String;

    fn from_str(value: &str) -> Result<GateNameArg, Self::Err> {
        Ok(GateNameArg::new(value.to_owned()))
    }
}

/// Clap arguments for `sotp gate-output`.
#[derive(Debug, Args)]
pub(crate) struct GateOutputArgs {
    /// Gate label forwarded to the summary and log-path construction.
    #[arg(long, default_value = "gate")]
    pub(crate) name: GateNameArg,

    /// Child command and arguments. Put `--` before the child command.
    #[arg(required = true, trailing_var_arg = true, value_name = "COMMAND")]
    pub(crate) command: Vec<OsString>,
}

/// Dispatches parsed gate arguments through the configured driver.
pub(crate) fn execute(args: GateOutputArgs) -> ExitCode {
    let GateOutputArgs { name, command } = args;
    let input = GateOutputInput::new(name.as_str().to_owned(), command);
    let outcome = {
        #[cfg(test)]
        {
            if let Some(driver) = TEST_DRIVER.with(|slot| slot.borrow_mut().take()) {
                driver.invoke(input)
            } else {
                GateOutputComposition::build(PathBuf::from(".")).invoke(input)
            }
        }
        #[cfg(not(test))]
        {
            GateOutputComposition::build(PathBuf::from(".")).invoke(input)
        }
    };
    if let Some(stdout) = outcome.stdout {
        println!("{stdout}");
    }
    if let Some(stderr) = outcome.stderr {
        eprintln!("{stderr}");
    }
    exit_code_from_u8(outcome.exit_code)
}

/// Converts the driver exit value to the process-boundary exit code.
#[must_use]
pub(crate) fn exit_code_from_u8(code: u8) -> ExitCode {
    ExitCode::from(code)
}

#[cfg(test)]
thread_local! {
    static TEST_DRIVER: std::cell::RefCell<Option<cli_driver::gate_output::GateOutputDriver>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
    use std::os::fd::AsRawFd as _;
    use std::path::PathBuf;

    #[cfg(unix)]
    use std::path::Path;
    #[cfg(unix)]
    use std::sync::Arc;

    use clap::{Parser, Subcommand};
    #[cfg(unix)]
    use infrastructure::gate_output::ProcessGateRunner;
    #[cfg(unix)]
    use usecase::gate_output::{
        GateAdapterFailureReason, GateLogPath, GateLogPersistencePort, GateLogReservation,
        GateLogReservationError, GateLogWriteError, GateProcessPort, GateRunCommand,
        GateRunInteractor, GateRunService,
    };

    use super::*;

    #[derive(Debug, Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: TestCommand,
    }

    #[derive(Debug, Subcommand)]
    enum TestCommand {
        GateOutput(GateOutputArgs),
    }

    #[cfg(unix)]
    const PREPARE_MARKER_ENV: &str = "SOTP_GATE_OUTPUT_PREPARE_MARKER";
    #[cfg(unix)]
    const LOG_PREFIX_ENV: &str = "SOTP_GATE_OUTPUT_LOG_PREFIX";
    #[cfg(unix)]
    const PREPARE_CHILD_TEST: &str =
        "commands::gate_output::tests::test_gate_output_child_writes_prepare_marker";
    #[cfg(unix)]
    const FAILURE_CHILD_TEST: &str =
        "commands::gate_output::tests::test_gate_output_child_removes_log_and_exits_23";
    #[cfg(unix)]
    const SUCCESS_CHILD_TEST: &str =
        "commands::gate_output::tests::test_gate_output_child_removes_log_and_exits_success";

    fn capture_stdout<T>(run: impl FnOnce() -> T) -> (T, String) {
        static STDOUT_REDIRECT: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _serialized = STDOUT_REDIRECT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut capture = tempfile::tempfile().expect("stdout capture should be created");
        let stdout_fd = std::io::stdout().as_raw_fd();
        std::io::stdout().flush().expect("stdout should be flushed");

        // Safety: stdout is a valid process file descriptor.
        let saved_fd = unsafe { libc::dup(stdout_fd) };
        assert!(saved_fd >= 0, "dup(stdout) failed");
        // Safety: both descriptors are valid; this redirects stdout to the capture file.
        let redirect_result = unsafe { libc::dup2(capture.as_raw_fd(), stdout_fd) };
        assert_eq!(redirect_result, stdout_fd, "dup2(capture, stdout) failed");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));

        std::io::stdout().flush().expect("captured stdout should be flushed");
        // Safety: saved_fd was returned by dup and restores the original stdout descriptor.
        let restore_result = unsafe { libc::dup2(saved_fd, stdout_fd) };
        assert_eq!(restore_result, stdout_fd, "dup2(saved, stdout) failed");
        // Safety: saved_fd is no longer needed after stdout has been restored.
        assert_eq!(unsafe { libc::close(saved_fd) }, 0, "close(saved stdout) failed");

        capture.seek(SeekFrom::Start(0)).expect("stdout capture should rewind");
        let mut output = String::new();
        capture.read_to_string(&mut output).expect("stdout capture should be readable");

        match result {
            Ok(value) => (value, output),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn reported_log_path(stdout: &str) -> PathBuf {
        let line = stdout
            .lines()
            .find(|line| line.starts_with("log: "))
            .expect("summary should report a log path");
        PathBuf::from(line.strip_prefix("log: ").expect("log line should have a prefix"))
    }

    #[cfg(unix)]
    fn child_test_command(test_name: &str) -> Vec<OsString> {
        vec![
            std::env::current_exe().expect("current test executable is available").into_os_string(),
            OsString::from("--exact"),
            OsString::from(test_name),
            OsString::from("--ignored"),
            OsString::from("--nocapture"),
        ]
    }

    #[cfg(unix)]
    fn remove_reserved_logs(prefix: &str) {
        for entry in std::fs::read_dir(Path::new("tmp/gate")).expect("gate log directory exists") {
            let entry = entry.expect("gate log entry should be readable");
            if entry.file_name().to_string_lossy().starts_with(prefix) {
                std::fs::remove_file(entry.path()).expect("reserved gate log should be removable");
            }
        }
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "invoked as a child-process fixture"]
    fn test_gate_output_child_writes_prepare_marker() {
        let marker = std::env::var_os(PREPARE_MARKER_ENV).expect("prepare marker path is set");
        std::fs::write(marker, b"child launched").expect("child marker should be written");
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "invoked as a child-process fixture"]
    #[allow(clippy::exit)]
    fn test_gate_output_child_removes_log_and_exits_23() {
        let prefix = std::env::var(LOG_PREFIX_ENV).expect("gate log prefix is set");
        remove_reserved_logs(&prefix);
        std::process::exit(23);
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "invoked as a child-process fixture"]
    fn test_gate_output_child_removes_log_and_exits_success() {
        let prefix = std::env::var(LOG_PREFIX_ENV).expect("gate log prefix is set");
        remove_reserved_logs(&prefix);
    }

    #[cfg(unix)]
    struct FailingReserveLogs;

    #[cfg(unix)]
    impl GateLogPersistencePort for FailingReserveLogs {
        fn reserve(
            &self,
            _command: &GateRunCommand,
        ) -> Result<GateLogReservation, GateLogReservationError> {
            Err(GateLogReservationError::CreateFile(GateAdapterFailureReason::new(
                "test reservation failure".to_owned(),
            )))
        }

        fn persist(
            &self,
            _reservation: GateLogReservation,
            _contents: &[u8],
        ) -> Result<GateLogPath, GateLogWriteError> {
            Err(GateLogWriteError::Write(GateAdapterFailureReason::new(
                "test persistence should not be reached".to_owned(),
            )))
        }
    }

    #[cfg(unix)]
    fn failing_reservation_driver() -> cli_driver::gate_output::GateOutputDriver {
        let runner: Arc<dyn GateProcessPort> = Arc::new(ProcessGateRunner::new());
        let logs: Arc<dyn GateLogPersistencePort> = Arc::new(FailingReserveLogs);
        let service: Arc<dyn GateRunService> = Arc::new(GateRunInteractor::new(runner, logs));
        cli_driver::gate_output::GateOutputDriver::new(service)
    }

    #[test]
    fn test_gate_output_args_parse_opaque_name_and_trailing_command() {
        let cli = TestCli::try_parse_from([
            "sotp",
            "gate-output",
            "--name",
            "leaf check",
            "--",
            "/bin/sh",
            "-c",
            "exit 23",
        ])
        .expect("gate-output arguments should parse");

        match cli.command {
            TestCommand::GateOutput(args) => {
                assert_eq!(args.name.as_str(), "leaf check");
                assert_eq!(args.command, ["/bin/sh", "-c", "exit 23"].map(OsString::from));
            }
        }
    }

    #[test]
    fn test_root_cli_command_dispatches_gate_output_log_and_exit_contract() {
        let cli = crate::Cli::try_parse_from([
            "sotp",
            "gate-output",
            "--name",
            "root-dispatch",
            "--",
            "/bin/sh",
            "-c",
            "printf '[FAIL] root-item: short reason\\n'; printf 'InternalRecord { stderr: true }\\n' >&2; exit 23",
        ])
        .expect("root gate-output command should parse");
        let args = match cli.command {
            Some(crate::CliCommand::GateOutput(args)) => args,
            _ => panic!("expected root gate-output command"),
        };

        let (exit, stdout) = capture_stdout(|| execute(args));

        assert_eq!(exit, ExitCode::from(23));
        let log_path = reported_log_path(&stdout);
        assert!(log_path.to_string_lossy().contains("tmp/gate/"));
        assert_eq!(
            std::fs::read(&log_path).expect("root dispatch should persist the complete log"),
            b"[FAIL] root-item: short reason\n--- stderr ---\nInternalRecord { stderr: true }\n"
        );
        assert_eq!(
            stdout,
            format!(
                "FAIL\nlog: {}\nfailures:\n- [FAIL] root-item: short reason\n",
                log_path.display()
            )
        );
    }

    #[test]
    fn test_execute_dispatches_child_exit_code_and_persists_log() {
        let name = format!("cli-execute-{}", std::process::id());
        let (exit, stdout) = capture_stdout(|| {
            execute(GateOutputArgs {
                name: GateNameArg::new(name),
                command: vec![
                    OsString::from("/bin/sh"),
                    OsString::from("-c"),
                    OsString::from(
                        "printf '[PASS] item-pass\\n[FAIL] item: short reason\\nInternalRecord { detail: true }\\n'; printf 'InternalRecord { stderr: true }\\n' >&2; exit 23",
                    ),
                ],
            })
        });

        assert_eq!(exit, ExitCode::from(23));
        let log_path = reported_log_path(&stdout);
        assert!(log_path.to_string_lossy().contains("tmp/gate/"));
        assert_eq!(
            std::fs::read(&log_path).expect("execute should persist the complete log"),
            b"[PASS] item-pass\n[FAIL] item: short reason\nInternalRecord { detail: true }\n--- stderr ---\nInternalRecord { stderr: true }\n"
        );
        assert_eq!(
            stdout,
            format!("FAIL\nlog: {}\nfailures:\n- [FAIL] item: short reason\n", log_path.display())
        );
        assert!(!stdout.contains("item-pass"));
        assert!(!stdout.contains("InternalRecord"));
        assert!(!stdout.contains("Debug"));
    }

    #[test]
    fn test_execute_dispatches_success_without_child_output_in_summary() {
        let (exit, stdout) = capture_stdout(|| {
            execute(GateOutputArgs {
                name: GateNameArg::new(format!("cli-success-{}", std::process::id())),
                command: vec![
                    OsString::from("/bin/sh"),
                    OsString::from("-c"),
                    OsString::from("printf '[PASS] item\\n'; printf 'DebugRecord\\n' >&2"),
                ],
            })
        });

        assert_eq!(exit, ExitCode::SUCCESS);
        let log_path = reported_log_path(&stdout);
        assert!(log_path.to_string_lossy().contains("tmp/gate/"));
        assert_eq!(
            std::fs::read(&log_path).expect("execute should persist the complete success log"),
            b"[PASS] item\n--- stderr ---\nDebugRecord\n"
        );
        assert_eq!(stdout, format!("PASS\nlog: {}\n", log_path.display()));
        assert!(!stdout.contains("[PASS] item"));
        assert!(!stdout.contains("DebugRecord"));
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_reports_prepare_failure_without_log_path_or_child_launch() {
        let marker_directory = tempfile::tempdir().expect("marker directory should be created");
        let marker = marker_directory.path().join("prepare-marker");
        let marker_value = marker.as_os_str().to_os_string();
        let (exit, stdout) = temp_env::with_var(PREPARE_MARKER_ENV, Some(marker_value), || {
            capture_stdout(|| {
                execute_with_driver(
                    failing_reservation_driver(),
                    GateOutputArgs {
                        name: GateNameArg::new("prepare-failure".to_owned()),
                        command: child_test_command(PREPARE_CHILD_TEST),
                    },
                )
            })
        });

        assert_eq!(exit, ExitCode::from(1));
        assert!(stdout.starts_with("FAIL\nlog unavailable: "));
        assert!(!stdout.lines().any(|line| line.starts_with("log: ")));
        assert!(!marker.exists());
    }

    /// Unix-only fixture: the child unlinks the still-open reservation to force
    /// the post-persist unavailable outcome; this relies on Unix unlink semantics.
    #[cfg(unix)]
    #[test]
    fn test_execute_preserves_child_status_when_log_persistence_is_unavailable() {
        let process_id = std::process::id();
        let failure_name = format!("t004-cli-post-persist-failure-{process_id}");
        let (failure_exit, failure_stdout) =
            temp_env::with_var(LOG_PREFIX_ENV, Some(failure_name.clone()), || {
                execute_command_gate(&failure_name, child_test_command(FAILURE_CHILD_TEST))
            });

        assert_eq!(failure_exit, ExitCode::from(23));
        assert!(failure_stdout.starts_with("FAIL\nlog unavailable: "));
        assert!(!failure_stdout.contains("failures:"));
        assert!(!failure_stdout.lines().any(|line| line.starts_with("log: ")));

        let success_name = format!("t004-cli-post-persist-success-{process_id}");
        let (success_exit, success_stdout) =
            temp_env::with_var(LOG_PREFIX_ENV, Some(success_name.clone()), || {
                execute_command_gate(&success_name, child_test_command(SUCCESS_CHILD_TEST))
            });

        assert_eq!(success_exit, ExitCode::SUCCESS);
        assert!(success_stdout.starts_with("PASS\nlog unavailable: "));
        assert!(!success_stdout.lines().any(|line| line.starts_with("log: ")));
    }

    #[cfg(unix)]
    fn execute_with_driver(
        driver: cli_driver::gate_output::GateOutputDriver,
        args: GateOutputArgs,
    ) -> ExitCode {
        let previous = super::TEST_DRIVER.with(|slot| slot.borrow_mut().replace(driver));
        assert!(previous.is_none(), "test driver slot should be empty");
        let exit = execute(args);
        super::TEST_DRIVER.with(|slot| {
            assert!(slot.borrow().is_none(), "test driver should be consumed by execute");
        });
        exit
    }

    #[cfg(unix)]
    fn execute_command_gate(name: &str, command: Vec<OsString>) -> (ExitCode, String) {
        capture_stdout(|| {
            execute(GateOutputArgs { name: GateNameArg::new(name.to_owned()), command })
        })
    }

    fn execute_shell_gate(name: &str, shell: &str) -> (ExitCode, String) {
        capture_stdout(|| {
            execute(GateOutputArgs {
                name: GateNameArg::new(name.to_owned()),
                command: vec![
                    OsString::from("/bin/sh"),
                    OsString::from("-c"),
                    OsString::from(shell),
                ],
            })
        })
    }

    #[test]
    fn test_test_execution_success_renders_pass_summary_and_preserves_log() {
        let (exit, stdout) = execute_shell_gate(
            &format!("t002-test-success-{}", std::process::id()),
            "printf 'test test_execution::passes ... ok\\ntest result: ok. 1 passed; 0 failed\\n'; printf 'DebugRecord { pass: true }\\n' >&2",
        );

        assert_eq!(exit, ExitCode::SUCCESS);
        let log_path = reported_log_path(&stdout);
        assert!(log_path.to_string_lossy().contains("tmp/gate/"));
        assert_eq!(
            std::fs::read(&log_path).expect("test execution should persist the complete log"),
            b"test test_execution::passes ... ok\ntest result: ok. 1 passed; 0 failed\n--- stderr ---\nDebugRecord { pass: true }\n"
        );
        assert_eq!(stdout, format!("PASS\nlog: {}\n", log_path.display()));
        assert!(!stdout.contains("test test_execution::passes"));
        assert!(!stdout.contains("DebugRecord"));
    }

    #[test]
    fn test_test_execution_failure_renders_failure_excerpt_and_preserves_exit_code() {
        let (exit, stdout) = execute_shell_gate(
            &format!("t002-test-failure-{}", std::process::id()),
            "printf 'test test_execution::fails ... FAILED\\ntest result: FAILED. 0 passed; 1 failed\\n'; printf 'InternalRecord { detail: true }\\n' >&2; exit 17",
        );

        assert_eq!(exit, ExitCode::from(17));
        let log_path = reported_log_path(&stdout);
        assert_eq!(
            std::fs::read(&log_path).expect("failed test execution should persist the full log"),
            b"test test_execution::fails ... FAILED\ntest result: FAILED. 0 passed; 1 failed\n--- stderr ---\nInternalRecord { detail: true }\n"
        );
        assert_eq!(
            stdout,
            format!(
                "FAIL\nlog: {}\nfailures:\n- test test_execution::fails ... FAILED\n- test result: FAILED. 0 passed; 1 failed\n",
                log_path.display()
            )
        );
        assert!(!stdout.contains("InternalRecord"));
    }

    #[test]
    fn test_obligation_check_success_renders_pass_summary_and_preserves_log() {
        let (exit, stdout) = execute_shell_gate(
            &format!("t002-obligation-success-{}", std::process::id()),
            "printf '[OK] test-obligation check passed: resolved_edges=69 uncited_findings=0\\n'; printf 'ObligationRecord { pass: true }\\n' >&2",
        );

        assert_eq!(exit, ExitCode::SUCCESS);
        let log_path = reported_log_path(&stdout);
        assert_eq!(
            std::fs::read(&log_path).expect("obligation check should persist the complete log"),
            b"[OK] test-obligation check passed: resolved_edges=69 uncited_findings=0\n--- stderr ---\nObligationRecord { pass: true }\n"
        );
        assert_eq!(stdout, format!("PASS\nlog: {}\n", log_path.display()));
        assert!(!stdout.contains("test-obligation check passed"));
        assert!(!stdout.contains("ObligationRecord"));
    }

    #[test]
    fn test_obligation_check_failure_renders_failure_excerpt_and_preserves_exit_code() {
        let (exit, stdout) = execute_shell_gate(
            &format!("t002-obligation-failure-{}", std::process::id()),
            "printf 'test-obligation check failed: missing binding\\n'; printf 'InternalRecord { detail: true }\\n' >&2; exit 19",
        );

        assert_eq!(exit, ExitCode::from(19));
        let log_path = reported_log_path(&stdout);
        assert_eq!(
            std::fs::read(&log_path).expect("failed obligation check should persist the full log"),
            b"test-obligation check failed: missing binding\n--- stderr ---\nInternalRecord { detail: true }\n"
        );
        assert_eq!(
            stdout,
            format!(
                "FAIL\nlog: {}\nfailures:\n- test-obligation check failed: missing binding\n",
                log_path.display()
            )
        );
        assert!(!stdout.contains("InternalRecord"));
    }

    #[test]
    fn test_exit_code_from_u8_preserves_driver_value() {
        assert_eq!(exit_code_from_u8(23), ExitCode::from(23));
        assert_eq!(exit_code_from_u8(0), ExitCode::SUCCESS);
    }
}
