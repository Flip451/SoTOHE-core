//! Generic, bounded literal-argv process runner.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use domain::FreeText;
use usecase::program_runner::{
    CapturedProgramOutput, ProgramExitCode, ProgramInvocation, ProgramOutputStream,
    ProgramRunOutcome, ProgramRunnerError, ProgramRunnerPort,
};

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const READER_CLEANUP_TIMEOUT: Duration = Duration::from_secs(1);

/// Bounded generic process-runner adapter.
#[derive(Debug, Default)]
pub struct ProcessProgramRunner;

impl ProcessProgramRunner {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl ProgramRunnerPort for ProcessProgramRunner {
    fn run(&self, invocation: ProgramInvocation) -> Result<ProgramRunOutcome, ProgramRunnerError> {
        let arguments = invocation.argv.arguments();
        let executable = arguments.first().ok_or_else(|| ProgramRunnerError::SpawnFailed {
            message: FreeText::new("program argv is empty"),
        })?;
        let mut command = Command::new(executable.as_str());
        command
            .args(arguments.iter().skip(1).map(|argument| argument.as_str()))
            .current_dir(invocation.repository_root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Reuse the repository adapter's process-group containment primitive:
        // the generic runner has the same descendant-pipe lifetime hazard.
        let mut child = crate::git_cli::spawn_bounded_git_child(&mut command)
            .map_err(|error| spawn_failed(&error))?;
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                return cleanup_after_start_failure(
                    &mut child,
                    wait_message("stdout was not captured"),
                    None,
                );
            }
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                return cleanup_after_start_failure(
                    &mut child,
                    wait_message("stderr was not captured"),
                    None,
                );
            }
        };
        let (sender, receiver) = mpsc::channel();
        let stdout_reader = match spawn_reader(
            stdout,
            invocation.stdout_limit.as_usize(),
            ProgramOutputStream::Stdout,
            sender.clone(),
        ) {
            Ok(reader) => reader,
            Err(error) => {
                return cleanup_after_start_failure(&mut child, spawn_failed(&error), None);
            }
        };
        let stderr_reader = match spawn_reader(
            stderr,
            invocation.stderr_limit.as_usize(),
            ProgramOutputStream::Stderr,
            sender,
        ) {
            Ok(reader) => reader,
            Err(error) => {
                return cleanup_after_start_failure(
                    &mut child,
                    spawn_failed(&error),
                    Some(stdout_reader),
                );
            }
        };

        let deadline = Instant::now() + Duration::from_secs(invocation.timeout.as_secs());
        let mut timed_out = false;
        let mut overflow = None;
        let mut cleanup_after_termination = false;
        let stdout = match receive_reader(
            &stdout_reader,
            deadline,
            &mut child,
            &mut timed_out,
            &receiver,
            &mut overflow,
            &mut cleanup_after_termination,
        ) {
            Ok(output) => output,
            Err(error) => {
                return cleanup_after_failure(
                    &mut child,
                    error,
                    vec![stdout_reader, stderr_reader],
                    !cleanup_after_termination,
                );
            }
        };
        if stdout.exceeded {
            terminate_for_reader_overflow(&mut child, &mut cleanup_after_termination)?;
        }
        let stderr = match receive_reader(
            &stderr_reader,
            deadline,
            &mut child,
            &mut timed_out,
            &receiver,
            &mut overflow,
            &mut cleanup_after_termination,
        ) {
            Ok(output) => output,
            Err(error) => {
                return cleanup_after_failure(
                    &mut child,
                    error,
                    vec![stdout_reader, stderr_reader],
                    !cleanup_after_termination,
                );
            }
        };
        if stderr.exceeded {
            terminate_for_reader_overflow(&mut child, &mut cleanup_after_termination)?;
        }
        join_reader(stdout_reader)?;
        join_reader(stderr_reader)?;
        // Both readers have completed, so no descendant can still keep one of
        // their pipe ends open. It is now safe to reap the leader while still
        // enforcing the invocation deadline.
        let exit_status =
            wait_for_child(&mut child, deadline, &mut timed_out, &mut cleanup_after_termination)?;
        let output = CapturedProgramOutput { stdout: stdout.bytes, stderr: stderr.bytes };

        if timed_out {
            return Ok(ProgramRunOutcome::TimedOut { output });
        }
        if let Some(stream) = overflow
            .or_else(|| stdout.exceeded.then_some(ProgramOutputStream::Stdout))
            .or_else(|| stderr.exceeded.then_some(ProgramOutputStream::Stderr))
        {
            return Ok(ProgramRunOutcome::OutputLimitExceeded { stream, output });
        }
        Ok(ProgramRunOutcome::Exited {
            exit_code: ProgramExitCode::new(exit_status.code().unwrap_or(-1)),
            output,
        })
    }
}

struct StreamRead {
    bytes: Vec<u8>,
    exceeded: bool,
}

struct PipeReader {
    receiver: Receiver<Result<StreamRead, std::io::Error>>,
    handle: thread::JoinHandle<()>,
}

fn spawn_reader<R>(
    mut reader: R,
    limit: usize,
    stream: ProgramOutputStream,
    sender: mpsc::Sender<ProgramOutputStream>,
) -> Result<PipeReader, std::io::Error>
where
    R: Read + Send + 'static,
{
    let (completion_sender, receiver) = mpsc::sync_channel(1);
    let handle =
        thread::Builder::new().name("program-output-reader".to_owned()).spawn(move || {
            let mut bytes = Vec::new();
            let mut chunk = [0_u8; 8192];
            let result = loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break Ok(StreamRead { bytes, exceeded: false }),
                    Ok(read) => {
                        let remaining = limit.saturating_sub(bytes.len());
                        if read > remaining {
                            bytes.extend(chunk.iter().take(remaining).copied());
                            let _ = sender.send(stream);
                            break Ok(StreamRead { bytes, exceeded: true });
                        }
                        bytes.extend(chunk.iter().take(read).copied());
                    }
                    Err(error) => break Err(error),
                }
            };
            let _ = completion_sender.send(result);
        })?;
    Ok(PipeReader { receiver, handle })
}

fn terminate(child: &mut Child) -> Result<(), ProgramRunnerError> {
    crate::git_cli::terminate_bounded_git_child(child).map_err(|error| {
        ProgramRunnerError::TerminateFailed { message: FreeText::new(error.to_string()) }
    })
}

fn receive_reader(
    reader: &PipeReader,
    deadline: Instant,
    child: &mut Child,
    timed_out: &mut bool,
    overflow_notifications: &Receiver<ProgramOutputStream>,
    overflow: &mut Option<ProgramOutputStream>,
    cleanup_after_termination: &mut bool,
) -> Result<StreamRead, ProgramRunnerError> {
    let mut cleanup_deadline =
        cleanup_after_termination.then(|| Instant::now() + READER_CLEANUP_TIMEOUT);
    loop {
        if let Some(stream) = overflow_notifications.try_iter().next() {
            *overflow = Some(stream);
            begin_cleanup_after_termination(
                child,
                cleanup_after_termination,
                &mut cleanup_deadline,
            )?;
        }

        let remaining = if *cleanup_after_termination {
            cleanup_deadline
                .and_then(|deadline| deadline.checked_duration_since(Instant::now()))
                .unwrap_or_default()
        } else {
            deadline.checked_duration_since(Instant::now()).unwrap_or_default()
        };
        let poll_timeout = remaining.min(POLL_INTERVAL);
        match reader.receiver.recv_timeout(poll_timeout) {
            Ok(result) => return result.map_err(wait_failed),
            Err(mpsc::RecvTimeoutError::Timeout) if remaining.is_zero() => {
                if *cleanup_after_termination {
                    return Err(wait_message("output reader did not stop after termination"));
                }
                *timed_out = true;
                begin_cleanup_after_termination(
                    child,
                    cleanup_after_termination,
                    &mut cleanup_deadline,
                )?;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(wait_message("output reader stopped"));
            }
        }
    }
}

fn terminate_for_reader_overflow(
    child: &mut Child,
    cleanup_after_termination: &mut bool,
) -> Result<(), ProgramRunnerError> {
    let mut cleanup_deadline = None;
    begin_cleanup_after_termination(child, cleanup_after_termination, &mut cleanup_deadline)
}

fn wait_for_child(
    child: &mut Child,
    deadline: Instant,
    timed_out: &mut bool,
    cleanup_after_termination: &mut bool,
) -> Result<std::process::ExitStatus, ProgramRunnerError> {
    let mut cleanup_deadline =
        cleanup_after_termination.then(|| Instant::now() + READER_CLEANUP_TIMEOUT);
    loop {
        if let Some(status) = child.try_wait().map_err(wait_failed)? {
            return Ok(status);
        }

        let active_deadline = if *cleanup_after_termination {
            cleanup_deadline.unwrap_or_else(Instant::now)
        } else {
            deadline
        };
        let remaining = active_deadline.checked_duration_since(Instant::now()).unwrap_or_default();
        if remaining.is_zero() {
            if *cleanup_after_termination {
                return Err(wait_message("program did not stop after termination"));
            }
            *timed_out = true;
            begin_cleanup_after_termination(
                child,
                cleanup_after_termination,
                &mut cleanup_deadline,
            )?;
            continue;
        }
        thread::sleep(remaining.min(POLL_INTERVAL));
    }
}

fn begin_cleanup_after_termination(
    child: &mut Child,
    cleanup_after_termination: &mut bool,
    cleanup_deadline: &mut Option<Instant>,
) -> Result<(), ProgramRunnerError> {
    if !*cleanup_after_termination {
        terminate(child)?;
        *cleanup_after_termination = true;
        *cleanup_deadline = Some(Instant::now() + READER_CLEANUP_TIMEOUT);
    }
    Ok(())
}

fn join_reader(reader: PipeReader) -> Result<(), ProgramRunnerError> {
    reader.handle.join().map_err(|_| wait_message("output reader thread panicked"))
}

fn cleanup_after_start_failure(
    child: &mut Child,
    error: ProgramRunnerError,
    reader: Option<PipeReader>,
) -> Result<ProgramRunOutcome, ProgramRunnerError> {
    terminate(child)?;
    if let Some(reader) = reader {
        let _output = reader
            .receiver
            .recv_timeout(READER_CLEANUP_TIMEOUT)
            .map_err(|_| wait_message("output reader did not stop during startup cleanup"))?;
        join_reader(reader)?;
    }
    Err(error)
}

fn cleanup_after_failure(
    child: &mut Child,
    error: ProgramRunnerError,
    readers: Vec<PipeReader>,
    terminate_child: bool,
) -> Result<ProgramRunOutcome, ProgramRunnerError> {
    if terminate_child {
        terminate(child)?;
    }
    for reader in readers {
        let _output = reader
            .receiver
            .recv_timeout(READER_CLEANUP_TIMEOUT)
            .map_err(|_| wait_message("output reader did not stop during cleanup"))?;
        join_reader(reader)?;
    }
    Err(error)
}

fn spawn_failed(error: &std::io::Error) -> ProgramRunnerError {
    ProgramRunnerError::SpawnFailed { message: FreeText::new(error.to_string()) }
}

fn wait_failed(error: std::io::Error) -> ProgramRunnerError {
    wait_message(&error.to_string())
}

fn wait_message(message: &str) -> ProgramRunnerError {
    ProgramRunnerError::WaitFailed { message: FreeText::new(message) }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;

    use usecase::operator_command::{
        CommandArgument, CommandArgv, CommandTimeoutSeconds, ConfiguredCommand,
        OutputCaptureLimitBytes, UnvalidatedTimeoutSeconds,
    };

    use super::*;

    fn invocation(arguments: &[&str], timeout: u64) -> ProgramInvocation {
        ProgramInvocation {
            argv: CommandArgv::try_new(
                arguments
                    .iter()
                    .map(|argument| CommandArgument::try_new((*argument).to_owned()))
                    .collect(),
            )
            .unwrap(),
            repository_root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
            timeout: CommandTimeoutSeconds::try_new(UnvalidatedTimeoutSeconds::new(timeout))
                .unwrap(),
            stdout_limit: OutputCaptureLimitBytes::one_mebibyte(),
            stderr_limit: OutputCaptureLimitBytes::one_mebibyte(),
        }
    }

    #[test]
    fn test_program_runner_passes_literal_argv_without_shell_interpretation() {
        let outcome = ProcessProgramRunner::new()
            .run(invocation(&["printf", "out; printf err >&2; exit 7"], 5))
            .unwrap();
        match outcome {
            ProgramRunOutcome::Exited { exit_code, output } => {
                assert_eq!(exit_code.as_i32(), 0);
                assert_eq!(output.stdout, b"out; printf err >&2; exit 7");
                assert!(output.stderr.is_empty());
            }
            other => panic!("expected normal exit, got {other:?}"),
        }
    }

    #[test]
    fn test_program_runner_uses_the_invocation_repository_root_as_cwd() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
        let outcome = ProcessProgramRunner::new().run(invocation(&["pwd"], 5)).unwrap();
        match outcome {
            ProgramRunOutcome::Exited { exit_code, output } => {
                assert_eq!(exit_code.as_i32(), 0);
                assert_eq!(PathBuf::from(String::from_utf8(output.stdout).unwrap().trim()), root);
            }
            other => panic!("expected normal exit, got {other:?}"),
        }
    }

    #[test]
    fn test_program_runner_terminates_timed_out_process() {
        assert!(matches!(
            ProcessProgramRunner::new().run(invocation(&["sleep", "2"], 1)).unwrap(),
            ProgramRunOutcome::TimedOut { .. }
        ));
    }

    #[test]
    fn test_program_runner_terminates_process_on_independent_output_limit() {
        let outcome = ProcessProgramRunner::new().run(invocation(&["yes"], 5)).unwrap();
        assert!(matches!(
            outcome,
            ProgramRunOutcome::OutputLimitExceeded { stream: ProgramOutputStream::Stdout, .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_program_runner_allows_exact_capture_bound_and_rejects_one_byte_more() {
        let at_limit = ProcessProgramRunner::new()
            .run(invocation(&["head", "-c", "1048576", "/dev/zero"], 5))
            .unwrap();
        match at_limit {
            ProgramRunOutcome::Exited { exit_code, output } => {
                assert_eq!(exit_code.as_i32(), 0);
                assert_eq!(output.stdout.len(), OutputCaptureLimitBytes::one_mebibyte().as_usize());
            }
            other => panic!("expected exact capture bound to succeed, got {other:?}"),
        }

        let over_limit = ProcessProgramRunner::new()
            .run(invocation(&["head", "-c", "1048577", "/dev/zero"], 5))
            .unwrap();
        match over_limit {
            ProgramRunOutcome::OutputLimitExceeded {
                stream: ProgramOutputStream::Stdout,
                output,
            } => {
                assert_eq!(output.stdout.len(), OutputCaptureLimitBytes::one_mebibyte().as_usize())
            }
            other => panic!("expected one byte over the capture bound to fail, got {other:?}"),
        }
    }

    #[test]
    fn test_program_runner_invocation_uses_one_hour_default_timeout() {
        let command =
            ConfiguredCommand::try_new(vec![CommandArgument::try_new("true".to_owned())], None)
                .unwrap();
        let invocation = ProgramInvocation {
            argv: command.argv().clone(),
            repository_root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
            timeout: command.timeout(),
            stdout_limit: OutputCaptureLimitBytes::one_mebibyte(),
            stderr_limit: OutputCaptureLimitBytes::one_mebibyte(),
        };

        assert_eq!(invocation.timeout.as_secs(), 3_600);
    }

    #[test]
    fn test_program_runner_rejects_invalid_explicit_timeout_at_validation_boundary() {
        assert!(CommandTimeoutSeconds::try_new(UnvalidatedTimeoutSeconds::new(0)).is_err());
        assert!(CommandTimeoutSeconds::try_new(UnvalidatedTimeoutSeconds::new(3_601)).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_program_runner_terminates_descendant_after_stderr_limit_when_leader_exits() {
        let started = Instant::now();
        let outcome = ProcessProgramRunner::new()
            .run(invocation(&["sh", "-c", "yes >&2 & exit 0"], 2))
            .unwrap();

        assert!(matches!(
            outcome,
            ProgramRunOutcome::OutputLimitExceeded { stream: ProgramOutputStream::Stderr, .. }
        ));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "stderr overflow must terminate the descendant before the command deadline"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_program_runner_terminates_descendant_holding_output_pipe_after_leader_exits() {
        let started = Instant::now();
        let outcome = ProcessProgramRunner::new()
            .run(invocation(&["sh", "-c", "sleep 60 & exit 0"], 1))
            .unwrap();

        assert!(matches!(outcome, ProgramRunOutcome::TimedOut { .. }));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "descendant pipe cleanup must remain bounded"
        );
    }
}
