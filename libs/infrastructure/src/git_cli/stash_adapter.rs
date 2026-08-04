//! Filesystem/Git adapter for the guarded stash boundary.

use std::io::Read;
use std::process::{Child, ExitStatus, Output, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use sha2::{Digest as _, Sha256};
use usecase::git_stash::{GitStashCommand, GitStashError, GitStashPort};
use usecase::git_workflow::DiagnosticText;

use super::{
    SystemGitRepo, collect_bounded_git_output, guarded_git_command, spawn_bounded_git_child,
};

const MAX_STASH_OUTPUT_BYTES: usize = 16 * 1024;
const STASH_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const STASH_POLL_INTERVAL: Duration = Duration::from_millis(10);
const STASH_PIPE_BUFFER_BYTES: usize = 8 * 1024;

/// Concrete Git adapter for [`GitStashPort`].
pub struct FsGitStashAdapter;

impl FsGitStashAdapter {
    /// Construct a guarded stash adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsGitStashAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BranchRefSnapshot {
    branch: Vec<u8>,
    head: Vec<u8>,
    branches: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GuardedStashSnapshot {
    branch_refs: BranchRefSnapshot,
    stash: Vec<u8>,
    worktree: Vec<u8>,
}

struct StashPipeReader<T> {
    receiver: Receiver<std::io::Result<T>>,
    handle: JoinHandle<()>,
}

fn unavailable(detail: impl Into<String>) -> GitStashError {
    GitStashError::Unavailable(DiagnosticText::new(detail))
}

fn run_git(repo: &SystemGitRepo, args: &[&str]) -> Result<Output, GitStashError> {
    let command_label = args.join(" ");
    let mut command = guarded_git_command();
    command
        .args(args)
        .current_dir(repo.root())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = spawn_bounded_git_child(&mut command)
        .map_err(|error| unavailable(format!("failed to spawn git {command_label}: {error}")))?;
    collect_bounded_git_output(child, MAX_STASH_OUTPUT_BYTES)
        .map_err(|error| unavailable(format!("failed to collect git {command_label}: {error}")))
}

fn output_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_owned()
}

fn command_failure(args: &[&str], output: &Output) -> GitStashError {
    let detail = output_text(output);
    let code = output
        .status
        .code()
        .map_or_else(|| "termination by signal".to_owned(), |code| format!("exit code {code}"));
    if detail.is_empty() {
        unavailable(format!("git {} failed with {code}", args.join(" ")))
    } else {
        unavailable(format!("git {} failed with {code}: {detail}", args.join(" ")))
    }
}

fn non_empty_output(output: &Output, description: &str) -> Result<Vec<u8>, GitStashError> {
    if output.stdout.iter().all(u8::is_ascii_whitespace) {
        return Err(unavailable(format!("git {description} returned an empty value")));
    }
    Ok(output.stdout.clone())
}

fn branch_ref_snapshot(repo: &SystemGitRepo) -> Result<BranchRefSnapshot, GitStashError> {
    let branch_args = ["rev-parse", "--abbrev-ref", "HEAD"];
    let branch = run_git(repo, &branch_args)?;
    if !branch.status.success() {
        return Err(command_failure(&branch_args, &branch));
    }

    let head_args = ["rev-parse", "--verify", "HEAD"];
    let head = run_git(repo, &head_args)?;
    if !head.status.success() {
        return Err(command_failure(&head_args, &head));
    }

    // A symbolic branch ref can point to a different branch without changing
    // its resolved object id. Hash the symbolic target and raw Git bytes so a
    // retarget cannot evade this guard through lossy Unicode conversion, and
    // stream them into a fixed-size digest so the snapshot stays bounded no
    // matter how many local heads the repository carries.
    let refs_args =
        ["for-each-ref", "--format=%(refname)%00%(objectname)%00%(symref)%00", "refs/heads"];
    let (refs_status, refs_digest, refs_stderr) = stream_git_output(repo, &refs_args)?;
    if !refs_status.success() {
        return Err(command_failure(
            &refs_args,
            &Output { status: refs_status, stdout: Vec::new(), stderr: refs_stderr },
        ));
    }

    Ok(BranchRefSnapshot {
        branch: non_empty_output(&branch, "rev-parse --abbrev-ref HEAD")?,
        head: non_empty_output(&head, "rev-parse --verify HEAD")?,
        branches: refs_digest,
    })
}

fn stash_ref_snapshot(repo: &SystemGitRepo) -> Result<Vec<u8>, GitStashError> {
    let args = ["rev-parse", "--verify", "--quiet", "refs/stash"];
    let output = run_git(repo, &args)?;
    if output.status.success() {
        return non_empty_output(&output, "rev-parse --verify refs/stash");
    }
    if output.status.code() == Some(1) {
        return Ok(Vec::new());
    }
    Err(command_failure(&args, &output))
}

fn spawn_digest_reader(
    mut pipe: impl Read + Send + 'static,
) -> std::io::Result<StashPipeReader<Vec<u8>>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle =
        thread::Builder::new().name("streaming-git-status-reader".to_owned()).spawn(move || {
            let mut hasher = Sha256::new();
            let mut buffer = [0_u8; STASH_PIPE_BUFFER_BYTES];
            let result = loop {
                match pipe.read(&mut buffer) {
                    Ok(0) => break Ok(hasher.finalize().to_vec()),
                    Ok(read) => {
                        let Some(chunk) = buffer.get(..read) else {
                            break Err(std::io::Error::other(
                                "git status reader returned an invalid byte count",
                            ));
                        };
                        hasher.update(chunk);
                    }
                    Err(error) => break Err(error),
                }
            };
            drop(pipe);
            let _ = sender.send(result);
        })?;
    Ok(StashPipeReader { receiver, handle })
}

fn spawn_bounded_stderr_reader(
    mut pipe: impl Read + Send + 'static,
) -> std::io::Result<StashPipeReader<Vec<u8>>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle = thread::Builder::new().name("bounded-git-status-stderr-reader".to_owned()).spawn(
        move || {
            let mut retained = Vec::new();
            let mut buffer = [0_u8; STASH_PIPE_BUFFER_BYTES];
            let result = loop {
                match pipe.read(&mut buffer) {
                    Ok(0) => break Ok(retained),
                    Ok(read) => {
                        let remaining = MAX_STASH_OUTPUT_BYTES.saturating_sub(retained.len());
                        let taken = read.min(remaining);
                        let Some(prefix) = buffer.get(..taken) else {
                            break Err(std::io::Error::other(
                                "git status stderr reader returned an invalid byte count",
                            ));
                        };
                        retained.extend_from_slice(prefix);
                        if taken < read {
                            break Err(std::io::Error::other(
                                "git status stderr exceeded its limit",
                            ));
                        }
                    }
                    Err(error) => break Err(error),
                }
            };
            drop(pipe);
            let _ = sender.send(result);
        },
    )?;
    Ok(StashPipeReader { receiver, handle })
}

fn wait_for_stash_child(child: &mut Child, started: Instant) -> std::io::Result<ExitStatus> {
    loop {
        match child.try_wait()? {
            Some(status) => return Ok(status),
            None if started.elapsed() >= STASH_COMMAND_TIMEOUT => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "git status timed out",
                ));
            }
            None => thread::sleep(STASH_POLL_INTERVAL),
        }
    }
}

fn receive_stash_reader<T>(reader: &StashPipeReader<T>, started: Instant) -> std::io::Result<T> {
    let remaining = STASH_COMMAND_TIMEOUT
        .checked_sub(started.elapsed())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::TimedOut, "git status timed out"))?;
    match reader.receiver.recv_timeout(remaining) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "git status timed out"))
        }
        Err(RecvTimeoutError::Disconnected) => {
            Err(std::io::Error::other("git status reader disconnected"))
        }
    }
}

fn join_stash_readers(readers: Vec<StashPipeReader<Vec<u8>>>) -> std::io::Result<()> {
    let mut first_error = None;
    for reader in readers {
        if reader.handle.join().is_err() && first_error.is_none() {
            first_error = Some(std::io::Error::other("git status reader panicked"));
        }
    }
    first_error.map_or(Ok(()), Err)
}

fn cleanup_stash_child(
    child: &mut Child,
    readers: Vec<StashPipeReader<Vec<u8>>>,
) -> std::io::Result<()> {
    let termination = super::terminate_bounded_git_child(child);
    let readers = join_stash_readers(readers);
    match (termination, readers) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(termination), Err(readers)) => Err(std::io::Error::other(format!(
            "git status cleanup failed ({termination}); reader cleanup failed ({readers})"
        ))),
    }
}

fn stream_git_output(
    repo: &SystemGitRepo,
    args: &[&str],
) -> Result<(ExitStatus, Vec<u8>, Vec<u8>), GitStashError> {
    let command_label = args.join(" ");
    let mut command = guarded_git_command();
    command
        .args(args)
        .current_dir(repo.root())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn_bounded_git_child(&mut command)
        .map_err(|error| unavailable(format!("failed to spawn git {command_label}: {error}")))?;
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let cleanup = cleanup_stash_child(&mut child, Vec::new());
            return Err(unavailable(format!(
                "failed to stream git {command_label} stdout: missing pipe; cleanup: {cleanup:?}"
            )));
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            let cleanup = cleanup_stash_child(&mut child, Vec::new());
            return Err(unavailable(format!(
                "failed to stream git {command_label} stderr: missing pipe; cleanup: {cleanup:?}"
            )));
        }
    };
    let stdout_reader = match spawn_digest_reader(stdout) {
        Ok(reader) => reader,
        Err(error) => {
            let cleanup = cleanup_stash_child(&mut child, Vec::new());
            return Err(unavailable(format!(
                "failed to start git {command_label} stdout reader: {error}; cleanup: {cleanup:?}"
            )));
        }
    };
    let stderr_reader = match spawn_bounded_stderr_reader(stderr) {
        Ok(reader) => reader,
        Err(error) => {
            let cleanup = cleanup_stash_child(&mut child, vec![stdout_reader]);
            return Err(unavailable(format!(
                "failed to start git {command_label} stderr reader: {error}; cleanup: {cleanup:?}"
            )));
        }
    };
    let readers = vec![stdout_reader, stderr_reader];
    let started = Instant::now();
    let status = match wait_for_stash_child(&mut child, started) {
        Ok(status) => status,
        Err(error) => {
            let cleanup = cleanup_stash_child(&mut child, readers);
            return Err(unavailable(format!(
                "failed to collect git {command_label}: {error}; cleanup: {cleanup:?}"
            )));
        }
    };
    let digest = match readers.first() {
        Some(reader) => match receive_stash_reader(reader, started) {
            Ok(digest) => digest,
            Err(error) => {
                let cleanup = cleanup_stash_child(&mut child, readers);
                return Err(unavailable(format!(
                    "failed to read git {command_label} stdout: {error}; cleanup: {cleanup:?}"
                )));
            }
        },
        None => {
            let cleanup = cleanup_stash_child(&mut child, readers);
            return Err(unavailable(format!(
                "git {command_label} stdout reader was missing; cleanup: {cleanup:?}"
            )));
        }
    };
    let stderr = match readers.get(1) {
        Some(reader) => match receive_stash_reader(reader, started) {
            Ok(stderr) => stderr,
            Err(error) => {
                let cleanup = cleanup_stash_child(&mut child, readers);
                return Err(unavailable(format!(
                    "failed to read git {command_label} stderr: {error}; cleanup: {cleanup:?}"
                )));
            }
        },
        None => {
            let cleanup = cleanup_stash_child(&mut child, readers);
            return Err(unavailable(format!(
                "git {command_label} stderr reader was missing; cleanup: {cleanup:?}"
            )));
        }
    };
    if let Err(error) = join_stash_readers(readers) {
        return Err(unavailable(format!("failed to join git {command_label} readers: {error}")));
    }
    Ok((status, digest, stderr))
}

fn worktree_snapshot(repo: &SystemGitRepo) -> Result<Vec<u8>, GitStashError> {
    let args = ["status", "--porcelain=v1", "-z", "--untracked-files=all"];
    let (status, digest, stderr) = stream_git_output(repo, &args)?;
    if !status.success() {
        return Err(command_failure(&args, &Output { status, stdout: Vec::new(), stderr }));
    }
    Ok(digest)
}

fn run_git_operation(repo: &SystemGitRepo, args: &[&str]) -> Result<Output, GitStashError> {
    let (status, _stdout_digest, stderr) = stream_git_output(repo, args)?;
    Ok(Output { status, stdout: Vec::new(), stderr })
}

fn guarded_stash_snapshot(repo: &SystemGitRepo) -> Result<GuardedStashSnapshot, GitStashError> {
    Ok(GuardedStashSnapshot {
        branch_refs: branch_ref_snapshot(repo)?,
        stash: stash_ref_snapshot(repo)?,
        worktree: worktree_snapshot(repo)?,
    })
}

fn re_adjudicate_after_uncertain_operation(
    repo: &SystemGitRepo,
    before: &GuardedStashSnapshot,
    operation: &[&str],
    reason: &GitStashError,
) -> GitStashError {
    match guarded_stash_snapshot(repo) {
        Ok(after) if before.branch_refs != after.branch_refs => {
            GitStashError::ForbiddenBranchRefUpdate
        }
        Ok(after) if before.stash != after.stash || before.worktree != after.worktree => {
            unavailable(format!(
                "git {} outcome is indeterminate after {reason}; stash or worktree state changed",
                operation.join(" ")
            ))
        }
        Ok(_) => unavailable(format!(
            "git {} outcome could not be confirmed after {reason}; do not retry without inspection",
            operation.join(" ")
        )),
        Err(inspect_error) => unavailable(format!(
            "git {} outcome could not be safely adjudicated after {reason}: {inspect_error}",
            operation.join(" ")
        )),
    }
}

impl GitStashPort for FsGitStashAdapter {
    fn execute(&self, command: GitStashCommand) -> Result<(), GitStashError> {
        let repo = SystemGitRepo::discover()
            .map_err(|error| unavailable(format!("cannot discover git repository: {error}")))?;
        let before = guarded_stash_snapshot(&repo)?;
        let args: &[&str] = match command {
            // `--include-untracked` is the long form of the ADR's `-u`
            // requirement and captures untracked track artifacts.
            GitStashCommand::Push => &["stash", "push", "--include-untracked"],
            GitStashCommand::Pop => &["stash", "pop"],
        };
        let output = match run_git_operation(&repo, args) {
            Ok(output) => output,
            Err(error) => {
                return Err(re_adjudicate_after_uncertain_operation(&repo, &before, args, &error));
            }
        };
        let after = match guarded_stash_snapshot(&repo) {
            Ok(after) => after,
            Err(error) => {
                return Err(re_adjudicate_after_uncertain_operation(&repo, &before, args, &error));
            }
        };

        if before.branch_refs != after.branch_refs {
            return Err(GitStashError::ForbiddenBranchRefUpdate);
        }
        if !output.status.success() {
            return Err(command_failure(args, &output));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::{Mutex, OnceLock};

    use super::{
        FsGitStashAdapter, MAX_STASH_OUTPUT_BYTES, guarded_stash_snapshot,
        re_adjudicate_after_uncertain_operation,
    };
    use crate::git_cli::SystemGitRepo;
    use usecase::git_stash::{GitStashCommand, GitStashError, GitStashPort};

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct CurrentDirGuard {
        original: PathBuf,
    }

    impl CurrentDirGuard {
        fn enter(path: &Path) -> Self {
            let original = std::env::current_dir().expect("test CWD must be readable");
            std::env::set_current_dir(path).expect("test must enter temporary repository");
            Self { original }
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.original).expect("test CWD must be restorable");
        }
    }

    fn run_git(root: &Path, args: &[&str]) {
        let status =
            Command::new("git").args(args).current_dir(root).status().expect("git must spawn");
        assert!(status.success(), "git {} failed", args.join(" "));
    }

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temporary repository must be created");
        run_git(dir.path(), &["init", "-q", "-b", "main"]);
        run_git(dir.path(), &["config", "user.email", "fixture@example.com"]);
        run_git(dir.path(), &["config", "user.name", "fixture"]);
        fs::write(dir.path().join("tracked.txt"), "base\n").expect("fixture file must be written");
        run_git(dir.path(), &["add", "tracked.txt"]);
        run_git(dir.path(), &["commit", "-m", "initial"]);
        dir
    }

    fn output(root: &Path, args: &[&str]) -> String {
        let result =
            Command::new("git").args(args).current_dir(root).output().expect("git must spawn");
        assert!(result.status.success(), "git {} failed", args.join(" "));
        String::from_utf8(result.stdout).expect("git output must be UTF-8")
    }

    fn branch_state(root: &Path) -> (String, String, String) {
        (
            output(root, &["rev-parse", "--abbrev-ref", "HEAD"]),
            output(root, &["rev-parse", "--verify", "HEAD"]),
            output(root, &["for-each-ref", "--format=%(refname)=%(objectname)", "refs/heads"]),
        )
    }

    #[test]
    fn test_fs_git_stash_adapter_push_includes_untracked_and_pop_restores_it() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "changed\n").expect("tracked file must change");
        fs::write(repo.path().join("untracked.txt"), "saved\n")
            .expect("untracked file must be written");

        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();
        adapter.execute(GitStashCommand::Push).expect("stash push must succeed");
        assert!(!repo.path().join("untracked.txt").exists());
        assert_eq!(output(repo.path(), &["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "main");

        adapter.execute(GitStashCommand::Pop).expect("stash pop must succeed");
        assert_eq!(
            fs::read_to_string(repo.path().join("tracked.txt")).expect("tracked file must exist"),
            "changed\n"
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("untracked.txt"))
                .expect("untracked file must be restored"),
            "saved\n"
        );
    }

    #[test]
    fn test_fs_git_stash_adapter_push_handles_large_worktree_snapshot() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "changed\n").expect("tracked file must change");
        let suffix = "x".repeat(48);
        for index in 0..400 {
            fs::write(
                repo.path().join(format!("untracked-artifact-{index:04}-{suffix}")),
                "saved\n",
            )
            .expect("large untracked fixture file must be written");
        }
        let status =
            output(repo.path(), &["status", "--porcelain=v1", "-z", "--untracked-files=all"]);
        assert!(
            status.len() > MAX_STASH_OUTPUT_BYTES,
            "fixture must exceed the former retained status bound"
        );
        let first_artifact = repo.path().join(format!("untracked-artifact-0000-{suffix}"));

        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();
        adapter.execute(GitStashCommand::Push).expect("large stash push must succeed");
        assert!(!first_artifact.exists());

        adapter.execute(GitStashCommand::Pop).expect("large stash pop must succeed");
        assert_eq!(
            fs::read_to_string(first_artifact).expect("large untracked file must be restored"),
            "saved\n"
        );
    }

    #[test]
    fn test_fs_git_stash_adapter_push_handles_large_branch_ref_snapshot() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        let suffix = "x".repeat(48);
        for index in 0..320 {
            run_git(repo.path(), &["branch", &format!("fixture-head-{index:04}-{suffix}")]);
        }
        let refs = output(
            repo.path(),
            &["for-each-ref", "--format=%(refname)%00%(objectname)%00%(symref)%00", "refs/heads"],
        );
        assert!(
            refs.len() > MAX_STASH_OUTPUT_BYTES,
            "fixture must exceed the former retained branch-ref bound"
        );
        fs::write(repo.path().join("untracked.txt"), "saved\n")
            .expect("untracked file must be written");

        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();
        adapter.execute(GitStashCommand::Push).expect("push must succeed with many local heads");
        adapter.execute(GitStashCommand::Pop).expect("pop must succeed with many local heads");
        assert_eq!(
            fs::read_to_string(repo.path().join("untracked.txt"))
                .expect("untracked file must be restored"),
            "saved\n"
        );
    }

    #[test]
    fn test_fs_git_stash_adapter_pop_without_saved_worktree_returns_unavailable() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        let _cwd = CurrentDirGuard::enter(repo.path());

        let error = FsGitStashAdapter::new()
            .execute(GitStashCommand::Pop)
            .expect_err("pop without a saved stash must fail closed");

        assert!(matches!(error, GitStashError::Unavailable(_)));
    }

    #[test]
    fn test_fs_git_stash_adapter_push_and_pop_preserve_head_and_branch_refs() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "changed\n").expect("tracked file must change");
        fs::write(repo.path().join("untracked.txt"), "saved\n")
            .expect("untracked file must be written");

        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();
        let before = branch_state(repo.path());

        adapter.execute(GitStashCommand::Push).expect("stash push must succeed");
        assert_eq!(branch_state(repo.path()), before);

        adapter.execute(GitStashCommand::Pop).expect("stash pop must succeed");
        assert_eq!(branch_state(repo.path()), before);
    }

    #[test]
    fn test_fs_git_stash_adapter_snapshot_detects_symbolic_branch_retarget() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        run_git(repo.path(), &["branch", "other", "main"]);
        run_git(repo.path(), &["symbolic-ref", "refs/heads/guarded", "refs/heads/main"]);
        let _cwd = CurrentDirGuard::enter(repo.path());
        let repository = SystemGitRepo::discover().expect("repository must be discoverable");

        let before = guarded_stash_snapshot(&repository).expect("snapshot must succeed");
        run_git(repo.path(), &["symbolic-ref", "refs/heads/guarded", "refs/heads/other"]);
        let after = guarded_stash_snapshot(&repository).expect("snapshot must succeed");

        assert_ne!(before.branch_refs, after.branch_refs);
    }

    #[test]
    fn test_fs_git_stash_adapter_uncertain_operation_detects_changed_worktree() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        let _cwd = CurrentDirGuard::enter(repo.path());
        let repository = SystemGitRepo::discover().expect("repository must be discoverable");
        let before = guarded_stash_snapshot(&repository).expect("snapshot must succeed");
        fs::write(repo.path().join("tracked.txt"), "changed\n")
            .expect("fixture file must be changed");
        let runner_error = super::unavailable("bounded runner timed out");

        let error = re_adjudicate_after_uncertain_operation(
            &repository,
            &before,
            &["stash", "push", "--include-untracked"],
            &runner_error,
        );

        assert!(matches!(error, GitStashError::Unavailable(_)));
        assert!(error.to_string().contains("stash or worktree state changed"));
    }
}
