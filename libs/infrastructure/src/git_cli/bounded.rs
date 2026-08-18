//! Bounded execution of a spawned Git child process.
//!
//! Part of the `git_cli` adapter, in its own file so that neither module
//! outgrows the workspace module-size limit. The runner is what keeps a Git
//! invocation from becoming unbounded in either dimension an external process
//! can grow in: the output it retains and the time it occupies.

use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const BOUNDED_GIT_TIMEOUT: Duration = Duration::from_secs(10);
const BOUNDED_GIT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const PROCESS_GROUP_KILL_TIMEOUT: Duration = Duration::from_secs(1);
const PROCESS_GROUP_PROBE_OUTPUT_LIMIT: usize = 1024;

/// Starts a command in a process group on Unix, so bounded cleanup can
/// terminate it as a unit with descendants which inherited an output pipe.
/// The supported execution environment for this adapter is Linux/WSL. Unix
/// process groups remain addressable after the direct child has exited.
#[cfg(unix)]
pub(crate) fn spawn_bounded_git_child(command: &mut Command) -> std::io::Result<Child> {
    configure_process_group(command);
    command.spawn()
}

/// No safe process-tree containment primitive is available for this target.
/// Refuse before spawning rather than leave a descendant holding a pipe.
#[cfg(not(unix))]
pub(crate) fn spawn_bounded_git_child(_command: &mut Command) -> std::io::Result<Child> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "bounded git output is not supported on this platform",
    ))
}

/// Collects a spawned Git command without retaining unbounded output or
/// allowing it to outlive the verification deadline.
///
/// One deadline governs the child's run and the wait for each reader's verdict,
/// and a reader that reaches its retention limit closes its pipe instead of
/// reading on, so no path here waits on a writer we cannot bound. Callers
/// classify the returned I/O error for their own public error surface; this
/// primitive deliberately carries no path-bearing stderr text.
pub(crate) fn collect_bounded_git_output(
    mut child: Child,
    max_output_bytes: usize,
) -> std::io::Result<Output> {
    let started = Instant::now();
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            return cleanup_after_error(
                std::io::Error::other("git stdout was not captured"),
                &mut child,
                Vec::new(),
            );
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            return cleanup_after_error(
                std::io::Error::other("git stderr was not captured"),
                &mut child,
                Vec::new(),
            );
        }
    };

    // A reader that fails to start leaves a child already running and, for the
    // second one, a reader already running. Neither may be abandoned: the child
    // would stay alive as an unreaped process, and the caller's deadline would
    // stop bounding anything.
    let stdout_reader = match spawn_bounded_pipe_reader(stdout, max_output_bytes, started) {
        Ok(reader) => reader,
        Err(error) => {
            return cleanup_after_error(error, &mut child, Vec::new());
        }
    };
    let stderr_reader = match spawn_bounded_pipe_reader(stderr, max_output_bytes, started) {
        Ok(reader) => reader,
        Err(error) => {
            return cleanup_after_error(error, &mut child, vec![stdout_reader]);
        }
    };

    let status = match wait_for_bounded_child(&mut child, started) {
        Ok(status) => status,
        Err(error) => {
            return cleanup_after_error(error, &mut child, vec![stdout_reader, stderr_reader]);
        }
    };
    let stdout = match receive_bounded_pipe(&stdout_reader.receiver, started) {
        Ok(output) => output,
        Err(error) => {
            return cleanup_after_error(error, &mut child, vec![stdout_reader, stderr_reader]);
        }
    };
    let stderr = match receive_bounded_pipe(&stderr_reader.receiver, started) {
        Ok(output) => output,
        Err(error) => {
            return cleanup_after_error(error, &mut child, vec![stdout_reader, stderr_reader]);
        }
    };
    // Both readers have delivered their one message, so each is at the end of
    // its closure: joining here costs nothing and leaves no thread behind.
    join_pipe_reader(stdout_reader)?;
    join_pipe_reader(stderr_reader)?;
    Ok(Output { status, stdout, stderr })
}

struct PipeReader {
    receiver: Receiver<std::io::Result<Vec<u8>>>,
    handle: JoinHandle<()>,
}

fn spawn_bounded_pipe_reader(
    mut pipe: impl std::io::Read + Send + 'static,
    max_output_bytes: usize,
    _started: Instant,
) -> std::io::Result<PipeReader> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle =
        thread::Builder::new().name("bounded-git-pipe-reader".to_owned()).spawn(move || {
            let mut retained = Vec::new();
            let mut buffer = [0_u8; 256];
            let result = loop {
                match pipe.read(&mut buffer) {
                    Ok(0) => break Ok(retained),
                    Ok(read) => {
                        let remaining = max_output_bytes.saturating_sub(retained.len());
                        let taken = read.min(remaining);
                        let Some(prefix) = buffer.get(..taken) else {
                            break Err(std::io::Error::other(
                                "git output reader returned an invalid byte count",
                            ));
                        };
                        retained.extend_from_slice(prefix);
                        if taken < read {
                            break Err(std::io::Error::other(
                                "git command output exceeded its limit",
                            ));
                        }
                    }
                    Err(error) => break Err(error),
                }
            };
            // Closing our end is what releases a writer that is blocked on a
            // full pipe, and it does so whether or not that writer is the child
            // we can reach. Reading on to the writer's own pace instead would
            // park this thread on a `read` no deadline can interrupt: an
            // inherited pipe held by a descendant never reaches end of file, and
            // one such call would strand the thread for as long as that
            // descendant lives. There is nothing left to gather here in any
            // case — the output is already refused.
            drop(pipe);
            let _ = sender.send(result);
        })?;
    Ok(PipeReader { receiver, handle })
}

fn wait_for_bounded_child(
    child: &mut Child,
    started: Instant,
) -> std::io::Result<std::process::ExitStatus> {
    loop {
        match child.try_wait()? {
            Some(status) => return Ok(status),
            None if started.elapsed() >= BOUNDED_GIT_TIMEOUT => {
                child.kill()?;
                reap_child_with_deadline(child, Instant::now())?;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "git command timed out",
                ));
            }
            None => thread::sleep(BOUNDED_GIT_POLL_INTERVAL),
        }
    }
}

fn receive_bounded_pipe(
    reader: &Receiver<std::io::Result<Vec<u8>>>,
    started: Instant,
) -> std::io::Result<Vec<u8>> {
    let remaining = BOUNDED_GIT_TIMEOUT.checked_sub(started.elapsed()).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::TimedOut, "git command timed out")
    })?;
    match reader.recv_timeout(remaining) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "git command timed out"))
        }
        Err(RecvTimeoutError::Disconnected) => {
            Err(std::io::Error::other("git command output could not be read"))
        }
    }
}

/// Terminates the child's process group, reaps the child, then releases any
/// reader already started. Killing the group closes pipe ends held by
/// descendants, allowing a reader blocked in `read` to finish before it is
/// dropped.
pub(crate) fn terminate_bounded_git_child(child: &mut Child) -> std::io::Result<()> {
    // An exited leader leaves its group empty, so group termination can return
    // ESRCH while `Child` still owns a zombie. Reaping must not depend on
    // termination succeeding; preserve both failures when they occur.
    let termination = terminate_process_group(child);
    let reaping = reap_child_with_deadline(child, Instant::now());
    match (termination, reaping) {
        (Ok(()), Ok(())) => Ok(()),
        // A reaped leader whose group is already empty is fully cleaned up:
        // group termination fails with "no such group" once every member is
        // gone, so verify emptiness before treating the kill as a failure.
        (Err(termination), Ok(())) => {
            if process_group_is_empty(child)? {
                Ok(())
            } else {
                Err(termination)
            }
        }
        (Ok(()), Err(error)) => Err(error),
        (Err(termination), Err(reaping)) => Err(std::io::Error::new(
            termination.kind(),
            format!(
                "git process cleanup failed (termination {:?}; reap {:?})",
                termination.kind(),
                reaping.kind()
            ),
        )),
    }
}

fn cleanup_after_error<T>(
    error: std::io::Error,
    child: &mut Child,
    readers: Vec<PipeReader>,
) -> std::io::Result<T> {
    match terminate_bounded_git_child(child) {
        Ok(()) => join_cleaned_readers(error, readers),
        Err(cleanup_error) => Err(std::io::Error::new(
            error.kind(),
            format!(
                "git output collection failed ({:?}); process cleanup failed ({:?})",
                error.kind(),
                cleanup_error.kind()
            ),
        )),
    }
}

#[cfg(unix)]
fn join_cleaned_readers<T>(error: std::io::Error, readers: Vec<PipeReader>) -> std::io::Result<T> {
    match readers.into_iter().try_for_each(join_pipe_reader) {
        Ok(()) => Err(error),
        Err(reader_error) => Err(std::io::Error::new(
            error.kind(),
            format!(
                "git output collection failed ({:?}); output reader cleanup failed ({:?})",
                error.kind(),
                reader_error.kind()
            ),
        )),
    }
}

#[cfg(not(unix))]
fn join_cleaned_readers<T>(error: std::io::Error, readers: Vec<PipeReader>) -> std::io::Result<T> {
    // Standard-library process termination is direct-child-only here. A
    // descendant may still hold a pipe, so joining its reader would abandon the
    // deadline. Dropping releases the receiver and lets a later EOF finish the
    // thread without ever blocking this caller.
    drop(readers);
    Err(error)
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
}

/// Probes whether the child's process group has no remaining members.
///
/// `kill -0` delivers no signal. Only its C-locale ESRCH diagnostic proves
/// that the group is absent; permission and all other errors fail closed.
#[cfg(unix)]
fn process_group_is_empty(child: &Child) -> std::io::Result<bool> {
    let process_group = format!("-{}", child.id());
    let mut command = Command::new("/bin/kill");
    command
        .args(["-0", "--", process_group.as_str()])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut probe = command.spawn()?;
    let stderr = probe
        .stderr
        .take()
        .ok_or_else(|| std::io::Error::other("process-group probe stderr was not captured"))?;
    let started = Instant::now();
    let reader = spawn_bounded_pipe_reader(stderr, PROCESS_GROUP_PROBE_OUTPUT_LIMIT, started)?;
    let status = wait_for_termination_command(probe)?;
    let stderr = receive_bounded_pipe(&reader.receiver, started)?;
    join_pipe_reader(reader)?;
    if status.success() {
        return Ok(false);
    }
    if stderr.ends_with(b": No such process\n") {
        return Ok(true);
    }
    Err(std::io::Error::other("could not determine whether git process group remains"))
}

/// Non-unix builds have no process-group probe; report the group as non-empty
/// so a failed group termination is never masked.
#[cfg(not(unix))]
fn process_group_is_empty(_child: &Child) -> std::io::Result<bool> {
    Ok(false)
}

#[cfg(unix)]
fn terminate_process_group(child: &mut Child) -> std::io::Result<()> {
    let process_group = format!("-{}", child.id());
    let mut command = Command::new("/bin/kill");
    command
        .args(["-KILL", "--", process_group.as_str()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let status = wait_for_termination_command(command.spawn()?)?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("git process group could not be terminated"))
    }
}

#[cfg(not(unix))]
fn terminate_process_group(_child: &mut Child) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "bounded git output is not supported on this platform",
    ))
}

#[cfg(unix)]
fn wait_for_termination_command(mut command: Child) -> std::io::Result<std::process::ExitStatus> {
    let started = Instant::now();
    loop {
        match command.try_wait()? {
            Some(status) => return Ok(status),
            None if started.elapsed() >= PROCESS_GROUP_KILL_TIMEOUT => {
                command.kill()?;
                reap_child_with_deadline(&mut command, Instant::now())?;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "git process group termination timed out",
                ));
            }
            None => thread::sleep(BOUNDED_GIT_POLL_INTERVAL),
        }
    }
}

fn join_pipe_reader(reader: PipeReader) -> std::io::Result<()> {
    reader.handle.join().map_err(|_| std::io::Error::other("git output reader panicked"))
}

fn reap_child_with_deadline(child: &mut Child, started: Instant) -> std::io::Result<()> {
    while started.elapsed() < BOUNDED_GIT_TIMEOUT {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        thread::sleep(BOUNDED_GIT_POLL_INTERVAL);
    }
    Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "git command could not be reaped"))
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{collect_bounded_git_output, spawn_bounded_git_child, terminate_bounded_git_child};

    #[cfg(unix)]
    #[test]
    fn test_the_bounded_git_runner_refuses_output_above_its_retention_limit() {
        // The discovery call routes through this runner. A deliberately noisy
        // child proves the reader rejects over-limit output without retaining
        // it, rather than relying on Command::output's unbounded buffers.
        let mut command = std::process::Command::new("sh");
        command
            .args(["-c", "i=0; while [ $i -le 1024 ]; do printf x; i=$((i + 1)); done"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let child = spawn_bounded_git_child(&mut command).unwrap();

        let error = collect_bounded_git_output(child, 1024)
            .expect_err("a command emitting more than the bounded limit must be refused");
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
    }

    #[cfg(unix)]
    #[test]
    fn test_an_over_limit_reader_does_not_wait_on_a_pipe_a_descendant_holds() {
        // The direct child exits immediately, but the descendant it leaves
        // behind inherits the pipe and holds it open far past the deadline.
        // Draining until the pipe closes would therefore park this reader for a
        // minute; the deadline is what makes the answer come back at all.
        let started = std::time::Instant::now();
        let mut command = std::process::Command::new("sh");
        command
            .args([
                "-c",
                "i=0; while [ $i -le 1024 ]; do printf x; i=$((i + 1)); done; sleep 60 & exit 0",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let child = spawn_bounded_git_child(&mut command).unwrap();

        let error = collect_bounded_git_output(child, 1024)
            .expect_err("output above the retention limit must be refused");

        // The over-limit refusal, not a timeout: the collection reached its own
        // conclusion within the deadline instead of being timed out by a pipe
        // nobody was going to close.
        assert_eq!(error.kind(), std::io::ErrorKind::Other, "unexpected: {error}");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "the refusal must not wait on a pipe the descendant still holds"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_the_bounded_git_runner_times_out_and_reaps_a_sleeping_child() {
        let started = std::time::Instant::now();
        let mut command = std::process::Command::new("sh");
        command
            .args(["-c", "sleep 60"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let child = spawn_bounded_git_child(&mut command).unwrap();

        let error = collect_bounded_git_output(child, 1024)
            .expect_err("a child that misses the deadline must be terminated");

        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(12),
            "timeout cleanup must not turn a ten-second deadline into an unbounded wait"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_timeout_terminates_a_descendant_that_keeps_an_output_pipe_open() {
        let pid_file = tempfile::NamedTempFile::new().unwrap();
        let pid_path = pid_file.path().to_owned();
        let mut command = std::process::Command::new("sh");
        command
            .args([
                "-c",
                "sleep 60 & printf '%s' \"$!\" > \"$1\"; exit 0",
                "--",
                pid_path.to_str().unwrap(),
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let child = spawn_bounded_git_child(&mut command).unwrap();

        let error = collect_bounded_git_output(child, 1024)
            .expect_err("an inherited output pipe must time out rather than strand a reader");
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);

        let descendant = std::fs::read_to_string(&pid_path).unwrap();
        let process_state = std::fs::read_to_string(format!("/proc/{}/stat", descendant.trim()))
            .ok()
            .and_then(|stat| stat.split_whitespace().nth(2).and_then(|state| state.chars().next()));
        assert!(
            !matches!(process_state, Some(state) if state != 'Z'),
            "cleanup must terminate the descendant holding the pipe, found {process_state:?}"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_cleanup_reaps_an_exited_child_when_group_termination_fails() {
        let mut command = std::process::Command::new("sh");
        command
            .args(["-c", "exit 0"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        // Deliberately do not put this child in a child-ID process group. The
        // cleanup helper's group signal therefore fails, while the exited
        // child still needs reaping. Production children use the group setup;
        // this fixture isolates the error-path invariant.
        let mut child = command.spawn().unwrap();
        let process_path = format!("/proc/{}", child.id());

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        loop {
            let state =
                std::fs::read_to_string(format!("{process_path}/stat")).ok().and_then(|stat| {
                    stat.split_whitespace().nth(2).and_then(|state| state.chars().next())
                });
            if state == Some('Z') {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the child must exit before its group-termination failure is exercised"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        terminate_bounded_git_child(&mut child).expect(
            "an exited child with an already-empty process group is fully cleaned up: the \
             failed group signal must not surface once the leader is reaped",
        );
        assert!(
            !std::path::Path::new(&process_path).exists(),
            "cleanup must reap the exited direct child even when group termination failed"
        );
    }
}
