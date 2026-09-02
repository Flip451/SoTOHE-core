use std::io::{Error, ErrorKind};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use super::output_reader::{self, NightlyReaderConfig};

pub(super) struct NightlyToolResolutionOutput {
    pub(super) status: ExitStatus,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}

pub(super) fn run_bounded_command_with_total(
    mut command: Command,
    maximum_stream_bytes: usize,
    maximum_total_bytes: u64,
    execution_timeout: Duration,
    drain_timeout: Duration,
    label: &str,
) -> Result<NightlyToolResolutionOutput, Error> {
    let started = Instant::now();
    let execution_deadline = started + execution_timeout;
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::capability_exec::process::configure_process_group(&mut command);
    let mut child = spawn_command_with_deadline(command, execution_deadline, label)?;
    let process_id = child.id();
    let stdout = child.stdout.take().ok_or_else(|| {
        Error::new(ErrorKind::BrokenPipe, format!("{label} stdout was not captured"))
    });
    let stdout = match stdout {
        Ok(stdout) => stdout,
        Err(error) => {
            let _ = terminate_nightly_process(&mut child, process_id);
            return Err(error);
        }
    };
    let stderr = child.stderr.take().ok_or_else(|| {
        Error::new(ErrorKind::BrokenPipe, format!("{label} stderr was not captured"))
    });
    let stderr = match stderr {
        Ok(stderr) => stderr,
        Err(error) => {
            let _ = terminate_nightly_process(&mut child, process_id);
            return Err(error);
        }
    };
    let shared_total = Arc::new(AtomicU64::new(0));
    let stdout_result: output_reader::NightlyOutputSlot = Arc::new(Mutex::new(None));
    let stderr_result: output_reader::NightlyOutputSlot = Arc::new(Mutex::new(None));
    let reader_config =
        NightlyReaderConfig { maximum_stream_bytes, maximum_total_bytes, execution_deadline };
    let stdout_generation = match output_reader::spawn_nightly_output_reader(
        stdout,
        Arc::clone(&stdout_result),
        Arc::clone(&shared_total),
        reader_config,
        label,
        "stdout",
    ) {
        Ok(generation) => generation,
        Err(error) => {
            let _ = terminate_nightly_process(&mut child, process_id);
            return Err(error);
        }
    };
    let stderr_generation = match output_reader::spawn_nightly_output_reader(
        stderr,
        Arc::clone(&stderr_result),
        shared_total,
        reader_config,
        label,
        "stderr",
    ) {
        Ok(generation) => generation,
        Err(error) => {
            let _ = terminate_nightly_process(&mut child, process_id);
            output_reader::retire_nightly_stdout_reader(stdout_generation);
            return Err(error);
        }
    };

    let mut exited_at = None;
    let mut status = None;
    let mut stdout = None;
    let mut stderr = None;
    loop {
        if stdout.is_none() {
            if let Some(result) = output_reader::take_nightly_output_result(&stdout_result) {
                match handle_nightly_output_result(result, &mut child, process_id, label) {
                    Ok(output) => stdout = Some(output),
                    Err(error) => {
                        output_reader::retire_nightly_reader_workers(
                            stdout_generation,
                            stderr_generation,
                        );
                        return Err(error);
                    }
                }
            }
        }
        if stderr.is_none() {
            if let Some(result) = output_reader::take_nightly_output_result(&stderr_result) {
                match handle_nightly_output_result(result, &mut child, process_id, label) {
                    Ok(output) => stderr = Some(output),
                    Err(error) => {
                        output_reader::retire_nightly_reader_workers(
                            stdout_generation,
                            stderr_generation,
                        );
                        return Err(error);
                    }
                }
            }
        }

        if status.is_none() {
            match child.try_wait() {
                Ok(Some(child_status)) => {
                    status = Some(child_status);
                    exited_at = Some(Instant::now());
                }
                Ok(None) => {}
                Err(error) => {
                    let termination_detail = terminate_nightly_process(&mut child, process_id)
                        .err()
                        .map(|error| format!("; process termination also failed: {error}"))
                        .unwrap_or_default();
                    output_reader::retire_nightly_reader_workers(
                        stdout_generation,
                        stderr_generation,
                    );
                    return Err(Error::new(
                        error.kind(),
                        format!("cannot poll {label}: {error}{termination_detail}"),
                    ));
                }
            }
        }

        if started.elapsed() >= execution_timeout {
            let termination_detail = terminate_nightly_process(&mut child, process_id)
                .err()
                .map(|error| format!("; process termination also failed: {error}"))
                .unwrap_or_default();
            output_reader::retire_nightly_reader_workers(stdout_generation, stderr_generation);
            return Err(Error::new(
                ErrorKind::TimedOut,
                format!("{label} timed out after {execution_timeout:?}{termination_detail}"),
            ));
        }
        if exited_at.is_some_and(|exited| exited.elapsed() >= drain_timeout) {
            let termination_detail = terminate_nightly_process(&mut child, process_id)
                .err()
                .map(|error| format!("; process termination also failed: {error}"))
                .unwrap_or_default();
            output_reader::retire_nightly_reader_workers(stdout_generation, stderr_generation);
            return Err(Error::new(
                ErrorKind::TimedOut,
                format!(
                    "{label} output drain timed out after {drain_timeout:?} following subprocess exit{termination_detail}"
                ),
            ));
        }
        if status.is_some() && stdout.is_some() && stderr.is_some() {
            let status = match status.take() {
                Some(status) => status,
                None => return Err(Error::other(format!("{label} lost its exit status"))),
            };
            let stdout = match stdout.take() {
                Some(stdout) => stdout,
                None => return Err(Error::other(format!("{label} lost stdout"))),
            };
            let stderr = match stderr.take() {
                Some(stderr) => stderr,
                None => return Err(Error::other(format!("{label} lost stderr"))),
            };
            return Ok(NightlyToolResolutionOutput { status, stdout, stderr });
        }
        thread::sleep(super::RUSTUP_WHICH_POLL_INTERVAL);
    }
}

const COMMAND_SPAWN_QUEUE_CAPACITY: usize = 1;
const COMMAND_SPAWN_QUEUE_POLL_INTERVAL: Duration = Duration::from_millis(5);
const MAX_COMMAND_SPAWN_WORKER_GENERATIONS: usize = 16;

type CommandSpawnJob = Box<dyn FnOnce() + Send + 'static>;

struct CommandSpawnWorker {
    generation: usize,
    sender: SyncSender<CommandSpawnJob>,
}

struct CommandSpawnExecutor {
    generations: usize,
    worker: Option<CommandSpawnWorker>,
}

struct CommandSpawnWorkerLease {
    generation: usize,
    sender: SyncSender<CommandSpawnJob>,
}

static COMMAND_SPAWN_EXECUTOR: OnceLock<Mutex<CommandSpawnExecutor>> = OnceLock::new();

fn spawn_command_worker(generation: usize) -> Result<SyncSender<CommandSpawnJob>, Error> {
    let (sender, receiver) = mpsc::sync_channel(COMMAND_SPAWN_QUEUE_CAPACITY);
    thread::Builder::new()
        .name(format!("sotp-command-spawn-worker-{generation}"))
        .spawn(move || {
            while let Ok(job) = receiver.recv() {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            }
        })
        .map(|_| sender)
        .map_err(|error| Error::other(format!("cannot spawn command worker: {error}")))
}

fn command_spawn_worker() -> Result<CommandSpawnWorkerLease, Error> {
    let executor = COMMAND_SPAWN_EXECUTOR
        .get_or_init(|| Mutex::new(CommandSpawnExecutor { generations: 0, worker: None }));
    let mut executor = match executor.lock() {
        Ok(executor) => executor,
        Err(poisoned) => poisoned.into_inner(),
    };
    if executor.worker.is_none() {
        if executor.generations >= MAX_COMMAND_SPAWN_WORKER_GENERATIONS {
            return Err(Error::new(
                ErrorKind::TimedOut,
                "command spawn worker restart budget exhausted",
            ));
        }
        let generation = executor.generations.saturating_add(1);
        let sender = spawn_command_worker(generation)?;
        executor.generations = generation;
        executor.worker = Some(CommandSpawnWorker { generation, sender });
    }
    let worker = match executor.worker.as_ref() {
        Some(worker) => worker,
        None => return Err(Error::other("command spawn worker is unavailable")),
    };
    Ok(CommandSpawnWorkerLease { generation: worker.generation, sender: worker.sender.clone() })
}

fn retire_command_spawn_worker(generation: usize) {
    let Some(executor) = COMMAND_SPAWN_EXECUTOR.get() else {
        return;
    };
    let mut executor = match executor.lock() {
        Ok(executor) => executor,
        Err(poisoned) => poisoned.into_inner(),
    };
    let is_current = executor.worker.as_ref().is_some_and(|worker| worker.generation == generation);
    if !is_current {
        return;
    }
    executor.worker = None;
    if executor.generations >= MAX_COMMAND_SPAWN_WORKER_GENERATIONS {
        return;
    }
    let next_generation = executor.generations.saturating_add(1);
    executor.generations = next_generation;
    if let Ok(sender) = spawn_command_worker(next_generation) {
        executor.worker = Some(CommandSpawnWorker { generation: next_generation, sender });
    }
}

struct CommandSpawnHandoff {
    cancelled: AtomicBool,
}

struct CommandSpawnChild {
    child: Option<Child>,
}

impl CommandSpawnChild {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn into_child(mut self) -> Result<Child, Error> {
        self.child.take().ok_or_else(|| Error::other("command spawn child was already taken"))
    }
}

impl Drop for CommandSpawnChild {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let process_id = child.id();
        let _ = terminate_nightly_process(&mut child, process_id);
    }
}

fn spawn_command_with_deadline(
    mut command: Command,
    execution_deadline: Instant,
    label: &str,
) -> Result<Child, Error> {
    let timeout_error = || {
        Error::new(
            ErrorKind::TimedOut,
            format!("{label} subprocess spawn timed out before execution deadline"),
        )
    };
    if execution_deadline.saturating_duration_since(Instant::now()).is_zero() {
        return Err(timeout_error());
    }

    let worker = command_spawn_worker()?;
    let (sender, receiver) = mpsc::sync_channel(1);
    let handoff = Arc::new(CommandSpawnHandoff { cancelled: AtomicBool::new(false) });
    let worker_handoff = Arc::clone(&handoff);
    let job: CommandSpawnJob = Box::new(move || {
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let _ = sender.send(Err(error));
                return;
            }
        };
        let child = CommandSpawnChild::new(child);
        if worker_handoff.cancelled.load(Ordering::Acquire) {
            drop(child);
            return;
        }
        if let Err(send_error) = sender.send(Ok(child))
            && let Ok(child) = send_error.0
        {
            drop(child);
        }
    });
    let mut job = Some(job);
    loop {
        let queued_job = match job.take() {
            Some(job) => job,
            None => return Err(Error::other("command spawn job was lost before scheduling")),
        };
        match worker.sender.try_send(queued_job) {
            Ok(()) => break,
            Err(TrySendError::Full(queued_job)) => {
                job = Some(queued_job);
                let remaining = execution_deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    cancel_command_spawn(&handoff, &receiver);
                    retire_command_spawn_worker(worker.generation);
                    return Err(timeout_error());
                }
                thread::sleep(COMMAND_SPAWN_QUEUE_POLL_INTERVAL.min(remaining));
            }
            Err(TrySendError::Disconnected(_)) => {
                cancel_command_spawn(&handoff, &receiver);
                retire_command_spawn_worker(worker.generation);
                return Err(Error::other("command spawn worker disconnected"));
            }
        }
    }

    let remaining = execution_deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        cancel_command_spawn(&handoff, &receiver);
        retire_command_spawn_worker(worker.generation);
        return Err(timeout_error());
    }
    match receiver.recv_timeout(remaining) {
        Ok(Ok(child)) => child.into_child(),
        Ok(Err(error)) => Err(error),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            cancel_command_spawn(&handoff, &receiver);
            retire_command_spawn_worker(worker.generation);
            Err(timeout_error())
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            retire_command_spawn_worker(worker.generation);
            Err(Error::other(format!("{label} spawn worker disconnected before returning a child")))
        }
    }
}

fn cancel_command_spawn(
    handoff: &CommandSpawnHandoff,
    receiver: &mpsc::Receiver<Result<CommandSpawnChild, Error>>,
) {
    handoff.cancelled.store(true, Ordering::Release);
    if let Ok(Ok(child)) = receiver.try_recv() {
        drop(child);
    }
}

#[cfg(test)]
fn run_nightly_tool_resolution(command: Command) -> Result<NightlyToolResolutionOutput, Error> {
    run_bounded_command_with_total(
        command,
        super::MAX_RUSTUP_WHICH_OUTPUT_BYTES,
        u64::MAX,
        super::super::EVALUATION_START_EXECUTION_TIMEOUT,
        super::super::EVALUATION_START_DRAIN_TIMEOUT,
        "rustup nightly tool resolution",
    )
}

pub(super) fn run_cargo_metadata_with_early_bounded_output(
    command: Command,
    maximum_stream_bytes: usize,
    maximum_total_bytes: u64,
    execution_timeout: Duration,
    drain_timeout: Duration,
) -> Result<crate::capability_exec::process::BoundedCommandOutput, Error> {
    let output = run_bounded_command_with_total(
        command,
        maximum_stream_bytes,
        maximum_total_bytes,
        execution_timeout,
        drain_timeout,
        "cargo metadata",
    )?;
    Ok(crate::capability_exec::process::BoundedCommandOutput {
        status: output.status,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn handle_nightly_output_result(
    result: output_reader::NightlyOutputResult,
    child: &mut Child,
    process_id: u32,
    label: &str,
) -> Result<Vec<u8>, Error> {
    result.map_err(|error| {
        let termination_detail = terminate_nightly_process(child, process_id)
            .err()
            .map(|error| format!("; process termination also failed: {error}"))
            .unwrap_or_default();
        Error::new(error.kind(), format!("{label} output failed: {error}{termination_detail}"))
    })
}

fn terminate_nightly_process(child: &mut Child, process_id: u32) -> Result<(), Error> {
    if crate::capability_exec::process::terminate_bounded_process_group(process_id).is_err() {
        if let Err(error) = child.kill() {
            if error.kind() != ErrorKind::InvalidInput {
                return Err(error);
            }
        }
    }
    child.wait().map(|_| ())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    #[cfg(unix)]
    use std::io::ErrorKind;
    #[cfg(unix)]
    use std::process::Command;
    #[cfg(unix)]
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    #[test]
    fn test_chatty_nightly_resolution_terminates_at_the_output_cap() {
        let blocks = super::super::MAX_RUSTUP_WHICH_OUTPUT_BYTES / 1024 + 1;
        let command_line =
            format!("/bin/dd if=/dev/zero bs=1024 count={blocks} 2>/dev/null; /bin/sleep 120");
        let mut command = Command::new("/bin/sh");
        command.args(["-c", command_line.as_str()]);
        let started = Instant::now();

        let error = match super::run_nightly_tool_resolution(command) {
            Ok(_) => panic!("chatty nightly resolution must fail at its output cap"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("exceeds"),
            "the output-cap failure must identify the bound: {error}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "output-cap termination must not wait for the 120-second command: {:?}",
            started.elapsed()
        );
    }
}
