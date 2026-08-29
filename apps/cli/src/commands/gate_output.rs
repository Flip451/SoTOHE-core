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
    let driver = GateOutputComposition::build(PathBuf::from("."));
    let outcome = driver.invoke(GateOutputInput::new(name.as_str().to_owned(), command));
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
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
    use std::os::fd::AsRawFd as _;
    use std::path::PathBuf;

    use clap::{Parser, Subcommand};

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
            "printf '[FAIL] root-item: short reason\\n'; printf 'full diagnostic detail\\n' >&2; exit 23",
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
            b"[FAIL] root-item: short reason\n--- stderr ---\nfull diagnostic detail\n"
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
                        "printf '[PASS] item-pass\\n[FAIL] item: short reason\\nfull diagnostic detail\\n'; printf 'stderr detail\\n' >&2; exit 23",
                    ),
                ],
            })
        });

        assert_eq!(exit, ExitCode::from(23));
        let log_path = reported_log_path(&stdout);
        assert!(log_path.to_string_lossy().contains("tmp/gate/"));
        assert_eq!(
            std::fs::read(&log_path).expect("execute should persist the complete log"),
            b"[PASS] item-pass\n[FAIL] item: short reason\nfull diagnostic detail\n--- stderr ---\nstderr detail\n"
        );
        assert_eq!(
            stdout,
            format!("FAIL\nlog: {}\nfailures:\n- [FAIL] item: short reason\n", log_path.display())
        );
        assert!(!stdout.contains("item-pass"));
        assert!(!stdout.contains("full diagnostic detail"));
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

    #[test]
    fn test_exit_code_from_u8_preserves_driver_value() {
        assert_eq!(exit_code_from_u8(23), ExitCode::from(23));
        assert_eq!(exit_code_from_u8(0), ExitCode::SUCCESS);
    }
}
