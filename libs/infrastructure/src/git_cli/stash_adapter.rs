//! Filesystem/Git adapter for the guarded stash boundary.
use std::path::PathBuf;
use std::process::{ExitStatus, Output, Stdio};
use std::time::Instant;

use domain::CommitHash;
use thiserror::Error;
use usecase::git_stash::{GitStashPopError, GitStashPort, GitStashPushError, GitStashPushOutcome};
use usecase::git_workflow::DiagnosticText;

use super::stash_record;
use super::stash_record::{
    StashRecord, cleanup_stash_child, join_stash_readers, receive_stash_reader,
    spawn_bounded_stderr_reader, spawn_digest_reader, wait_for_stash_child,
};
use super::{
    SystemGitRepo, collect_bounded_git_output, guarded_git_command, spawn_bounded_git_child,
};

pub(crate) use super::stash_record::MAX_STASH_OUTPUT_BYTES;
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

#[derive(Debug, Error)]
enum StashOperationError {
    #[error("git stash unavailable: {0}")]
    Unavailable(DiagnosticText),
}

impl From<StashOperationError> for GitStashPushError {
    fn from(error: StashOperationError) -> Self {
        match error {
            StashOperationError::Unavailable(detail) => Self::Unavailable(detail),
        }
    }
}

impl From<StashOperationError> for GitStashPopError {
    fn from(error: StashOperationError) -> Self {
        match error {
            StashOperationError::Unavailable(detail) => Self::Unavailable(detail),
        }
    }
}

fn unavailable(detail: impl Into<String>) -> StashOperationError {
    StashOperationError::Unavailable(DiagnosticText::new(detail))
}

fn record_error(detail: impl Into<String>) -> StashOperationError {
    unavailable(format!("guarded stash pairing record unavailable: {}", detail.into()))
}

fn pop_record_error(detail: impl Into<String>) -> GitStashPopError {
    GitStashPopError::Unavailable(DiagnosticText::new(format!(
        "guarded stash pairing record unavailable: {}",
        detail.into()
    )))
}

fn git_directory(
    repo: &SystemGitRepo,
    directory_flag: &str,
    description: &str,
) -> Result<PathBuf, StashOperationError> {
    let args = ["rev-parse", "--path-format=absolute", directory_flag];
    let output = run_git(repo, &args)?;
    if !output.status.success() {
        return Err(command_failure(&args, &output));
    }
    let path_text = String::from_utf8(output.stdout).map_err(|error| {
        unavailable(format!("git rev-parse returned an invalid {description}: {error}"))
    })?;
    let path_text = path_text.trim();
    if path_text.is_empty() {
        return Err(unavailable(format!("git rev-parse returned an empty {description}")));
    }
    let path = PathBuf::from(path_text);
    let path = if path.is_absolute() { path } else { repo.root().join(path) };
    crate::track::symlink_guard::reject_symlinks_up_to_root(&path).map_err(|error| {
        unavailable(format!("{description} contains an unsafe path component: {error}"))
    })?;
    let canonical = path
        .canonicalize()
        .map_err(|error| unavailable(format!("cannot canonicalize {description}: {error}")))?;
    if !canonical.is_dir() {
        return Err(unavailable(format!("{description} is not a directory")));
    }
    Ok(canonical)
}

fn stash_state_dir(repo: &SystemGitRepo) -> Result<PathBuf, StashOperationError> {
    git_directory(repo, "--absolute-git-dir", "worktree Git directory")
}

fn stash_common_dir(repo: &SystemGitRepo) -> Result<PathBuf, StashOperationError> {
    git_directory(repo, "--git-common-dir", "Git common directory")
}

fn stash_lock(
    repo: &SystemGitRepo,
) -> Result<stash_record::StashOperationLock, StashOperationError> {
    let path = stash_common_dir(repo)?.join(stash_record::STASH_LOCK_FILE);
    stash_record::acquire_lock(&path).map_err(record_error)
}

fn run_git(repo: &SystemGitRepo, args: &[&str]) -> Result<Output, StashOperationError> {
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

fn stream_git_output(
    repo: &SystemGitRepo,
    args: &[&str],
) -> Result<(ExitStatus, Vec<u8>, Vec<u8>), StashOperationError> {
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
    join_stash_readers(readers).map_err(|error| {
        unavailable(format!("failed to join git {command_label} readers: {error}"))
    })?;
    Ok((status, digest, stderr))
}

fn pending_record(repo: &SystemGitRepo) -> Result<StashRecord, GitStashPopError> {
    let state_dir = stash_state_dir(repo)?;
    stash_record::read(&state_dir)
        .map_err(pop_record_error)?
        .ok_or(GitStashPopError::NoPendingGuardedStash)
}

fn persist_record(repo: &SystemGitRepo, record: &StashRecord) -> Result<(), StashOperationError> {
    let state_dir = stash_state_dir(repo)?;
    stash_record::write(&state_dir, record).map_err(|error| {
        let detail = match &record.outcome {
            GitStashPushOutcome::Created(oid) => format!(
                "{error}; created stash OID {} was not paired — recover manually with `git stash list` and `git stash apply {}`",
                oid.as_ref(),
                oid.as_ref()
            ),
            GitStashPushOutcome::NothingToStash => error,
        };
        record_error(detail)
    })
}

fn clear_record(repo: &SystemGitRepo, record: &StashRecord) -> Result<(), GitStashPopError> {
    let state_dir = stash_state_dir(repo)?;
    stash_record::clear(&state_dir, record).map_err(pop_record_error)
}

fn output_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_owned()
}

fn command_failure(args: &[&str], output: &Output) -> StashOperationError {
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

fn non_empty_output(output: &Output, description: &str) -> Result<Vec<u8>, StashOperationError> {
    if output.stdout.iter().all(u8::is_ascii_whitespace) {
        return Err(unavailable(format!("git {description} returned an empty value")));
    }
    Ok(output.stdout.clone())
}

fn branch_ref_snapshot(repo: &SystemGitRepo) -> Result<BranchRefSnapshot, StashOperationError> {
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

fn stash_ref_snapshot(repo: &SystemGitRepo) -> Result<Vec<u8>, StashOperationError> {
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

fn created_stash_outcome(stash: &[u8]) -> Result<GitStashPushOutcome, StashOperationError> {
    let identity = String::from_utf8(stash.to_vec())
        .map_err(|error| unavailable(format!("git created an invalid stash identity: {error}")))?;
    let identity = identity.trim().to_owned();
    if identity.is_empty() {
        return Err(unavailable("git created a stash without a commit identity"));
    }
    CommitHash::try_new(identity)
        .map(GitStashPushOutcome::Created)
        .map_err(|error| unavailable(format!("git created an invalid stash identity: {error}")))
}

fn apply_recorded_stash(
    repo: &SystemGitRepo,
    expected: &CommitHash,
) -> Result<(), GitStashPopError> {
    let before = branch_ref_snapshot(repo)?;
    let args = ["stash", "apply", expected.as_ref()];
    let output = run_git(repo, &args)?;
    if !output.status.success() {
        let detail = output_text(&output);
        if detail.contains("not a valid reference")
            || detail.contains("unknown revision")
            || detail.contains("does not exist")
            || detail.contains("not a stash-like commit")
        {
            return Err(GitStashPopError::StashIdentityMissing(expected.clone()));
        }
        return Err(command_failure(&args, &output).into());
    }
    let after = branch_ref_snapshot(repo)?;
    if before != after {
        return Err(GitStashPopError::ForbiddenBranchRefUpdate);
    }
    Ok(())
}

impl GitStashPort for FsGitStashAdapter {
    fn push(&self) -> Result<GitStashPushOutcome, GitStashPushError> {
        let repo = SystemGitRepo::discover()
            .map_err(|error| unavailable(format!("cannot discover git repository: {error}")))?;
        let _lock = stash_lock(&repo)?;
        let state_dir = stash_state_dir(&repo)?;
        if stash_record::read(&state_dir).map_err(record_error)?.is_some() {
            return Err(GitStashPushError::PendingGuardedStashExists);
        }
        let before_refs = branch_ref_snapshot(&repo)?;
        let before_stash = stash_ref_snapshot(&repo)?;
        let args = ["stash", "push", "--include-untracked", "--message", "sotp-guarded"];
        let output = run_git(&repo, &args)?;
        if !output.status.success() {
            return Err(command_failure(&args, &output).into());
        }
        let after_stash = stash_ref_snapshot(&repo)?;
        let outcome = if before_stash == after_stash {
            GitStashPushOutcome::NothingToStash
        } else {
            created_stash_outcome(&after_stash)?
        };
        // Persist the pairing record before the ref comparison so a drift
        // failure never leaves a created stash without its pop pairing.
        let record = StashRecord::new(outcome);
        persist_record(&repo, &record)?;
        let after_refs = branch_ref_snapshot(&repo)?;
        if before_refs != after_refs {
            return Err(GitStashPushError::ForbiddenBranchRefUpdate);
        }
        Ok(record.outcome.clone())
    }

    fn pop(&self) -> Result<(), GitStashPopError> {
        let repo = SystemGitRepo::discover()
            .map_err(|error| unavailable(format!("cannot discover git repository: {error}")))?;
        let _lock = stash_lock(&repo)?;
        let persisted = pending_record(&repo)?;
        match &persisted.outcome {
            GitStashPushOutcome::NothingToStash => clear_record(&repo, &persisted),
            GitStashPushOutcome::Created(expected) => {
                apply_recorded_stash(&repo, expected)?;
                clear_record(&repo, &persisted)
            }
        }
    }
}
#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::{FsGitStashAdapter, MAX_STASH_OUTPUT_BYTES, stash_common_dir, stash_state_dir};
    use crate::git_cli::SystemGitRepo;
    use crate::git_cli::stash_record::{self, STASH_LOCK_FILE, STASH_RECORD_FILE, StashRecord};
    use domain::CommitHash;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use usecase::git_stash::{
        GitStashPopError, GitStashPort, GitStashPushError, GitStashPushOutcome,
    };

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
        run_git(dir.path(), &["commit", "-q", "-m", "initial"]);
        dir
    }

    fn output(root: &Path, args: &[&str]) -> String {
        let result =
            Command::new("git").args(args).current_dir(root).output().expect("git must spawn");
        assert!(result.status.success(), "git {} failed", args.join(" "));
        String::from_utf8(result.stdout).expect("git output must be UTF-8")
    }

    #[cfg(unix)]
    fn write_executable(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;

        fs::write(path, body).expect("hook must be written");
        let mut permissions =
            fs::metadata(path).expect("hook metadata must be readable").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("hook must be executable");
    }

    fn record_path(root: &Path) -> PathBuf {
        let git_dir = PathBuf::from(
            output(root, &["rev-parse", "--path-format=absolute", "--absolute-git-dir"]).trim(),
        );
        git_dir.join(STASH_RECORD_FILE)
    }

    fn lock_path(root: &Path) -> PathBuf {
        let _cwd = CurrentDirGuard::enter(root);
        let repository = SystemGitRepo::discover().expect("repository must be discoverable");
        stash_common_dir(&repository)
            .expect("common Git directory must be readable")
            .join(STASH_LOCK_FILE)
    }

    fn branch_state(root: &Path) -> (String, String, String) {
        (
            output(root, &["rev-parse", "--abbrev-ref", "HEAD"]),
            output(root, &["rev-parse", "--verify", "HEAD"]),
            output(
                root,
                &[
                    "for-each-ref",
                    "--format=%(refname)%00%(objectname)%00%(symref)%00",
                    "refs/heads",
                ],
            ),
        )
    }

    #[test]
    fn test_fs_git_stash_adapter_second_push_with_pending_record_fails_closed() {
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "saved\n")
            .expect("stash change must be written");
        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();

        adapter.push().expect("guarded push must succeed");
        fs::write(repo.path().join("tracked.txt"), "second\n")
            .expect("second change must be written");

        assert!(matches!(adapter.push(), Err(GitStashPushError::PendingGuardedStashExists)));
        assert!(record_path(repo.path()).is_file(), "the pending record must be retained");
    }

    #[test]
    fn test_fs_git_stash_adapter_linked_worktree_does_not_read_other_worktree_record() {
        let repo = init_repo();
        let linked = repo.path().join("linked");
        let linked_text = linked.to_str().expect("linked worktree path must be UTF-8");
        run_git(repo.path(), &["worktree", "add", "-q", "-b", "linked", linked_text]);

        fs::write(repo.path().join("tracked.txt"), "saved in main\n")
            .expect("stash change must be written");
        {
            let _cwd = CurrentDirGuard::enter(repo.path());
            FsGitStashAdapter::new().push().expect("main worktree stash must succeed");
        }

        let main_record = record_path(repo.path());
        let linked_record = record_path(&linked);
        let main_lock = lock_path(repo.path());
        let linked_lock = lock_path(&linked);
        assert!(main_record.is_file(), "main worktree must retain its pairing record");
        assert_ne!(main_record, linked_record, "linked worktrees need separate record paths");
        assert_eq!(main_lock, linked_lock, "linked worktrees must share the stash mutation lock");
        assert!(!linked_record.exists(), "linked worktree must not see main's record");

        let error = {
            let _cwd = CurrentDirGuard::enter(&linked);
            FsGitStashAdapter::new().pop().expect_err("linked pop must not consume main's record")
        };
        assert!(matches!(error, GitStashPopError::NoPendingGuardedStash));
        assert!(main_record.is_file(), "main worktree pairing record must remain pending");
    }

    #[test]
    fn test_fs_git_stash_adapter_push_records_oid_and_pop_applies_only_that_oid() {
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "saved\n")
            .expect("stash change must be written");
        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();

        let outcome = adapter.push().expect("guarded push must succeed");
        let expected = match &outcome {
            GitStashPushOutcome::Created(identity) => identity.clone(),
            GitStashPushOutcome::NothingToStash => panic!("fixture must create a stash"),
        };
        assert_eq!(fs::read_to_string(repo.path().join("tracked.txt")).unwrap(), "base\n");
        assert!(record_path(repo.path()).is_file());

        adapter.pop().expect("guarded pop must succeed");
        assert_eq!(fs::read_to_string(repo.path().join("tracked.txt")).unwrap(), "saved\n");
        assert!(!record_path(repo.path()).exists());
        assert!(
            output(repo.path(), &["stash", "list", "--format=%H"])
                .lines()
                .any(|line| line == expected.as_ref()),
            "the paired stash OID must remain independently addressable"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_git_stash_adapter_branch_ref_drift_after_push_retains_pairing_record() {
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "saved\n")
            .expect("stash change must be written");
        let hooks = repo.path().join(".githooks");
        fs::create_dir_all(&hooks).expect("hook directory must be created");
        write_executable(
            &hooks.join("reference-transaction"),
            "#!/bin/sh\nset -eu\nif [ \"$1\" = committed ] && ! git show-ref --verify --quiet refs/heads/stash-drift; then\n    git update-ref refs/heads/stash-drift HEAD\nfi\n",
        );
        run_git(repo.path(), &["config", "core.hooksPath", ".githooks"]);

        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();
        let error = adapter.push().expect_err("branch-ref drift must fail closed");

        assert!(matches!(error, GitStashPushError::ForbiddenBranchRefUpdate));
        let repository = SystemGitRepo::discover().expect("repository must be discoverable");
        let state_dir = stash_state_dir(&repository).expect("state directory must be readable");
        let persisted = stash_record::read(&state_dir)
            .expect("pairing record must be readable")
            .expect("created stash must retain its pairing record");
        assert!(matches!(persisted.outcome, GitStashPushOutcome::Created(_)));
        assert!(record_path(repo.path()).is_file(), "the pairing record must be retained");
        assert!(
            output(repo.path(), &["show-ref", "--verify", "refs/heads/stash-drift"])
                .lines()
                .any(|line| line.ends_with("refs/heads/stash-drift")),
            "the hook must create the branch-ref drift"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_git_stash_adapter_persist_failure_reports_created_oid_recovery() {
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "saved\n")
            .expect("stash change must be written");
        let hooks = repo.path().join(".githooks");
        fs::create_dir_all(&hooks).expect("hook directory must be created");
        write_executable(
            &hooks.join("reference-transaction"),
            "#!/bin/sh\nset -eu\nif [ \"$1\" = committed ]; then\n    git_dir=$(git rev-parse --absolute-git-dir)\n    record=\"$git_dir/.sotp-guarded-stash.json\"\n    if [ ! -e \"$record\" ]; then\n        mkdir \"$record\"\n    fi\nfi\n",
        );
        run_git(repo.path(), &["config", "core.hooksPath", ".githooks"]);

        let error = {
            let _cwd = CurrentDirGuard::enter(repo.path());
            FsGitStashAdapter::new().push().expect_err("record persistence must fail")
        };
        let message = match error {
            GitStashPushError::Unavailable(detail) => detail.as_str().to_owned(),
            other => panic!("expected persistence failure, got {other:?}"),
        };
        let stash_list = output(repo.path(), &["stash", "list", "--format=%H"]);
        let oid = stash_list.lines().next().expect("the successful stash must remain available");
        assert!(message.contains(oid), "the created stash OID must be reported: {message}");
        assert!(
            message.contains("git stash list"),
            "manual listing recovery is required: {message}"
        );
        assert!(
            message.contains(&format!("git stash apply {oid}")),
            "manual apply recovery is required: {message}"
        );
    }

    #[test]
    fn test_fs_git_stash_adapter_untracked_track_artifact_preserves_refs_and_unrelated_stash() {
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "unrelated stash\n")
            .expect("unrelated stash change must be written");
        run_git(repo.path(), &["stash", "push", "-q", "--message", "unrelated"]);
        let unrelated_stash = output(repo.path(), &["stash", "list", "--format=%H"])
            .lines()
            .next()
            .expect("unrelated stash must have an OID")
            .to_owned();
        let artifact = repo.path().join("track/items/fixture/untracked.txt");
        fs::create_dir_all(artifact.parent().expect("artifact parent must exist"))
            .expect("track artifact directory must be created");
        fs::write(&artifact, "untracked track artifact\n")
            .expect("untracked track artifact must be written");
        let before = branch_state(repo.path());
        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();

        let outcome = adapter.push().expect("guarded push must include untracked artifacts");
        let expected = match &outcome {
            GitStashPushOutcome::Created(identity) => identity.clone(),
            GitStashPushOutcome::NothingToStash => panic!("fixture must create a stash"),
        };
        assert_eq!(branch_state(repo.path()), before);
        let repository = SystemGitRepo::discover().expect("repository must be discoverable");
        let state_dir = stash_state_dir(&repository).expect("state directory must be readable");
        let persisted = stash_record::read(&state_dir)
            .expect("pairing record must be readable")
            .expect("created stash must have a pairing record");
        assert_eq!(persisted.outcome, outcome);
        assert!(fs::read_to_string(record_path(repo.path())).unwrap().contains(expected.as_ref()));
        assert!(!artifact.exists(), "the untracked artifact must be stashed");

        adapter.pop().expect("guarded pop must restore the recorded stash");

        assert_eq!(fs::read_to_string(&artifact).unwrap(), "untracked track artifact\n");
        assert_eq!(branch_state(repo.path()), before);
        assert!(!record_path(repo.path()).exists());
        let stash_oids = output(repo.path(), &["stash", "list", "--format=%H"]);
        assert!(stash_oids.lines().any(|line| line == expected.as_ref()));
        assert!(stash_oids.lines().any(|line| line == unrelated_stash));
    }

    #[test]
    fn test_fs_git_stash_adapter_pop_applies_recorded_stash_when_unrelated_stash_is_on_top() {
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "guarded stash\n")
            .expect("guarded stash change must be written");
        let artifact = repo.path().join("track/items/fixture/untracked.txt");
        fs::create_dir_all(artifact.parent().expect("artifact parent must exist"))
            .expect("track artifact directory must be created");
        fs::write(&artifact, "guarded artifact\n").expect("guarded artifact must be written");
        let before = branch_state(repo.path());
        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();

        let outcome = adapter.push().expect("guarded push must succeed");
        let recorded = match &outcome {
            GitStashPushOutcome::Created(identity) => identity.clone(),
            GitStashPushOutcome::NothingToStash => panic!("fixture must create a stash"),
        };

        fs::write(repo.path().join("tracked.txt"), "unrelated top stash\n")
            .expect("unrelated top stash change must be written");
        run_git(repo.path(), &["stash", "push", "-q", "--message", "unrelated-top"]);
        let unrelated_top = output(repo.path(), &["stash", "list", "--format=%H"])
            .lines()
            .next()
            .expect("unrelated top stash must have an OID")
            .to_owned();

        assert_ne!(unrelated_top, recorded.as_ref());
        adapter.pop().expect("guarded pop must select its recorded stash");

        assert_eq!(fs::read_to_string(repo.path().join("tracked.txt")).unwrap(), "guarded stash\n");
        assert_eq!(fs::read_to_string(&artifact).unwrap(), "guarded artifact\n");
        assert_eq!(branch_state(repo.path()), before);
        assert!(!record_path(repo.path()).exists());
        let stash_oids = output(repo.path(), &["stash", "list", "--format=%H"]);
        assert_eq!(stash_oids.lines().next(), Some(unrelated_top.as_str()));
        assert!(stash_oids.lines().any(|line| line == recorded.as_ref()));
    }

    #[test]
    fn test_fs_git_stash_adapter_nothing_to_stash_records_and_clears_without_touching_stack() {
        let repo = init_repo();
        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();

        assert_eq!(
            adapter.push().expect("clean guarded push must succeed"),
            GitStashPushOutcome::NothingToStash
        );
        assert!(record_path(repo.path()).is_file());
        adapter.pop().expect("no-stash pop must clear its record");
        assert!(!record_path(repo.path()).exists());
        assert!(output(repo.path(), &["stash", "list"]).trim().is_empty());
    }

    #[test]
    fn test_fs_git_stash_adapter_push_handles_large_branch_ref_snapshot() {
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
            "fixture must exceed the retained Git output bound"
        );
        fs::write(repo.path().join("untracked.txt"), "saved\n")
            .expect("untracked file must be written");

        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();
        adapter.push().expect("push must succeed with many local heads");
        adapter.pop().expect("pop must succeed with many local heads");
        assert_eq!(
            fs::read_to_string(repo.path().join("untracked.txt"))
                .expect("untracked file must be restored"),
            "saved\n"
        );
    }

    #[test]
    fn test_fs_git_stash_adapter_pop_without_record_fails_closed() {
        let repo = init_repo();
        let _cwd = CurrentDirGuard::enter(repo.path());

        let error = FsGitStashAdapter::new()
            .pop()
            .expect_err("pop without a pairing record must fail closed");
        assert!(matches!(error, GitStashPopError::NoPendingGuardedStash));
    }

    #[test]
    fn test_fs_git_stash_adapter_missing_recorded_oid_fails_without_clearing_record() {
        let repo = init_repo();
        let _cwd = CurrentDirGuard::enter(repo.path());
        let repository = SystemGitRepo::discover().expect("repository must be discoverable");
        let state_dir = stash_state_dir(&repository).expect("state directory must be readable");
        let expected = CommitHash::try_new("0123456789abcdef0123456789abcdef01234567").unwrap();
        stash_record::write(
            &state_dir,
            &StashRecord::new(GitStashPushOutcome::Created(expected.clone())),
        )
        .expect("fixture record must be written");

        let error =
            FsGitStashAdapter::new().pop().expect_err("a missing recorded OID must fail closed");
        assert!(
            matches!(error, GitStashPopError::StashIdentityMissing(identity) if identity == expected)
        );
        assert!(record_path(repo.path()).is_file());
    }
}
