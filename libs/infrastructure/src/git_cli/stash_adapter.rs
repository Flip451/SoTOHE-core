//! Filesystem/Git adapter for the guarded stash boundary.

use std::process::{Output, Stdio};

use usecase::git_stash::{GitStashCommand, GitStashError, GitStashPort};
use usecase::git_workflow::DiagnosticText;

use super::{
    SystemGitRepo, collect_bounded_git_output, guarded_git_command, spawn_bounded_git_child,
};

const MAX_STASH_OUTPUT_BYTES: usize = 16 * 1024;

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
    // its resolved object id. Keep the symbolic target and raw Git bytes so a
    // retarget cannot evade this guard through lossy Unicode conversion.
    let refs_args =
        ["for-each-ref", "--format=%(refname)%00%(objectname)%00%(symref)%00", "refs/heads"];
    let branches = run_git(repo, &refs_args)?;
    if !branches.status.success() {
        return Err(command_failure(&refs_args, &branches));
    }

    Ok(BranchRefSnapshot {
        branch: non_empty_output(&branch, "rev-parse --abbrev-ref HEAD")?,
        head: non_empty_output(&head, "rev-parse --verify HEAD")?,
        branches: branches.stdout,
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

fn worktree_snapshot(repo: &SystemGitRepo) -> Result<Vec<u8>, GitStashError> {
    let args = ["status", "--porcelain=v1", "-z", "--untracked-files=all"];
    let output = run_git(repo, &args)?;
    if !output.status.success() {
        return Err(command_failure(&args, &output));
    }
    Ok(output.stdout)
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
        let output = match run_git(&repo, args) {
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
        FsGitStashAdapter, guarded_stash_snapshot, re_adjudicate_after_uncertain_operation,
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
