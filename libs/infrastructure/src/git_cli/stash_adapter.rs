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
    StashListSummary, StashReaderKind, StashReaderResult, StashRecord, StashWorktreeIdentity,
    cleanup_stash_child, join_stash_readers, receive_stash_reader, spawn_stash_reader,
    wait_for_stash_child,
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
#[derive(Debug, Clone, PartialEq, Eq)]
struct GuardedStashSnapshot {
    branch_refs: BranchRefSnapshot,
    stash: Vec<u8>,
    worktree: Vec<u8>,
}
#[derive(Debug, Error)]
enum StashOperationError {
    #[error("guarded stash attempted a forbidden branch-ref update")]
    ForbiddenBranchRefUpdate,
    #[error("git stash unavailable: {0}")]
    Unavailable(DiagnosticText),
}
impl From<StashOperationError> for GitStashPushError {
    fn from(error: StashOperationError) -> Self {
        match error {
            StashOperationError::ForbiddenBranchRefUpdate => Self::ForbiddenBranchRefUpdate,
            StashOperationError::Unavailable(detail) => Self::Unavailable(detail),
        }
    }
}
impl From<StashOperationError> for GitStashPopError {
    fn from(error: StashOperationError) -> Self {
        match error {
            StashOperationError::ForbiddenBranchRefUpdate => Self::ForbiddenBranchRefUpdate,
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
fn stash_state_dir(repo: &SystemGitRepo) -> Result<PathBuf, StashOperationError> {
    let args = ["rev-parse", "--path-format=absolute", "--git-common-dir"];
    let output = run_git(repo, &args)?;
    if !output.status.success() {
        return Err(command_failure(&args, &output));
    }
    let path_text = String::from_utf8(output.stdout).map_err(|error| {
        unavailable(format!("git rev-parse returned an invalid Git common directory: {error}"))
    })?;
    let path_text = path_text.trim();
    if path_text.is_empty() {
        return Err(unavailable("git rev-parse returned an empty Git common directory"));
    }
    let path = PathBuf::from(path_text);
    let path = if path.is_absolute() { path } else { repo.root().join(path) };
    crate::track::symlink_guard::reject_symlinks_up_to_root(&path).map_err(|error| {
        unavailable(format!("Git common directory contains an unsafe path component: {error}"))
    })?;
    let canonical = path.canonicalize().map_err(|error| {
        unavailable(format!("cannot canonicalize Git common directory: {error}"))
    })?;
    if !canonical.is_dir() {
        return Err(unavailable("Git common directory is not a directory"));
    }
    Ok(canonical)
}

fn worktree_identity(repo: &SystemGitRepo) -> Result<StashWorktreeIdentity, StashOperationError> {
    let git_dir_args = ["rev-parse", "--absolute-git-dir"];
    let git_dir_output = run_git(repo, &git_dir_args)?;
    if !git_dir_output.status.success() {
        return Err(command_failure(&git_dir_args, &git_dir_output));
    }
    let git_dir_text = String::from_utf8(git_dir_output.stdout).map_err(|error| {
        unavailable(format!("git rev-parse returned an invalid Git directory: {error}"))
    })?;
    let git_dir_text = git_dir_text.trim();
    if git_dir_text.is_empty() {
        return Err(unavailable("git rev-parse returned an empty Git directory"));
    }
    let git_dir = PathBuf::from(git_dir_text);
    let git_dir = if git_dir.is_absolute() { git_dir } else { repo.root().join(git_dir) };
    crate::track::symlink_guard::reject_symlinks_up_to_root(&git_dir).map_err(|error| {
        unavailable(format!("Git directory contains an unsafe path component: {error}"))
    })?;
    let git_dir = git_dir
        .canonicalize()
        .map_err(|error| unavailable(format!("cannot canonicalize Git directory: {error}")))?;
    if !git_dir.is_dir() {
        return Err(unavailable("Git directory is not a directory"));
    }
    let git_dir =
        git_dir.to_str().ok_or_else(|| unavailable("Git directory is not valid UTF-8"))?.to_owned();

    crate::track::symlink_guard::reject_symlinks_up_to_root(repo.root()).map_err(|error| {
        unavailable(format!("Git worktree root contains an unsafe path component: {error}"))
    })?;
    let canonical = repo
        .root()
        .canonicalize()
        .map_err(|error| unavailable(format!("cannot canonicalize Git worktree root: {error}")))?;
    if !canonical.is_dir() {
        return Err(unavailable("Git worktree root is not a directory"));
    }
    let value = canonical
        .to_str()
        .ok_or_else(|| unavailable("Git worktree root is not valid UTF-8"))?
        .to_owned();
    StashWorktreeIdentity::try_new(git_dir, value).map_err(unavailable)
}

fn stash_lock(
    repo: &SystemGitRepo,
) -> Result<stash_record::StashOperationLock, StashOperationError> {
    let path = stash_state_dir(repo)?.join(stash_record::STASH_LOCK_FILE);
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
fn stash_list_snapshot(
    repo: &SystemGitRepo,
    expected: &CommitHash,
) -> Result<StashListSummary, StashOperationError> {
    let args = ["stash", "list", "--format=%H"];
    let (status, result, stderr) =
        stream_git_output_with_kind(repo, &args, StashReaderKind::List(expected.clone()))?;
    if !status.success() {
        return Err(command_failure(&args, &Output { status, stdout: Vec::new(), stderr }));
    }
    match result {
        StashReaderResult::List(summary) => Ok(summary),
        StashReaderResult::Digest(_) | StashReaderResult::Bounded(_) => {
            Err(unavailable("git stash list returned an unexpected reader result"))
        }
    }
}
fn pending_record(repo: &SystemGitRepo) -> Result<StashRecord, GitStashPopError> {
    let state_dir = stash_state_dir(repo)?;
    stash_record::read(&state_dir)
        .map_err(pop_record_error)?
        .ok_or(GitStashPopError::NoPendingGuardedStash)
}
fn persist_record(repo: &SystemGitRepo, record: &StashRecord) -> Result<(), StashOperationError> {
    let state_dir = stash_state_dir(repo)?;
    stash_record::write(&state_dir, record).map_err(record_error)
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

fn stream_git_output_with_kind(
    repo: &SystemGitRepo,
    args: &[&str],
    kind: StashReaderKind,
) -> Result<(ExitStatus, StashReaderResult, Vec<u8>), StashOperationError> {
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
    let stdout_reader = match spawn_stash_reader(stdout, kind) {
        Ok(reader) => reader,
        Err(error) => {
            let cleanup = cleanup_stash_child(&mut child, Vec::new());
            return Err(unavailable(format!(
                "failed to start git {command_label} stdout reader: {error}; cleanup: {cleanup:?}"
            )));
        }
    };
    let stderr_reader = match spawn_stash_reader(stderr, StashReaderKind::Bounded) {
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
    let stdout_result = match readers.first() {
        Some(reader) => match receive_stash_reader(reader, started) {
            Ok(result) => result,
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
            Ok(StashReaderResult::Bounded(stderr)) => stderr,
            Ok(_) => {
                let cleanup = cleanup_stash_child(&mut child, readers);
                return Err(unavailable(format!(
                    "git {command_label} stderr reader returned an unexpected result; cleanup: {cleanup:?}"
                )));
            }
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
    Ok((status, stdout_result, stderr))
}

fn stream_git_output(
    repo: &SystemGitRepo,
    args: &[&str],
) -> Result<(ExitStatus, Vec<u8>, Vec<u8>), StashOperationError> {
    let (status, result, stderr) =
        stream_git_output_with_kind(repo, args, StashReaderKind::Digest)?;
    match result {
        StashReaderResult::Digest(digest) => Ok((status, digest, stderr)),
        StashReaderResult::Bounded(_) | StashReaderResult::List(_) => {
            Err(unavailable("git command returned an unexpected stdout reader result"))
        }
    }
}
fn worktree_snapshot(repo: &SystemGitRepo) -> Result<Vec<u8>, StashOperationError> {
    let args = ["status", "--porcelain=v1", "-z", "--untracked-files=all"];
    let (status, digest, stderr) = stream_git_output(repo, &args)?;
    if !status.success() {
        return Err(command_failure(&args, &Output { status, stdout: Vec::new(), stderr }));
    }
    Ok(digest)
}
fn run_git_operation(repo: &SystemGitRepo, args: &[&str]) -> Result<Output, StashOperationError> {
    let (status, _stdout_digest, stderr) = stream_git_output(repo, args)?;
    Ok(Output { status, stdout: Vec::new(), stderr })
}
fn guarded_stash_snapshot(
    repo: &SystemGitRepo,
) -> Result<GuardedStashSnapshot, StashOperationError> {
    Ok(GuardedStashSnapshot {
        branch_refs: branch_ref_snapshot(repo)?,
        stash: stash_ref_snapshot(repo)?,
        worktree: worktree_snapshot(repo)?,
    })
}
fn push_outcome(
    before: &GuardedStashSnapshot,
    after: &GuardedStashSnapshot,
) -> Result<GitStashPushOutcome, StashOperationError> {
    if before.stash == after.stash {
        if before.worktree != after.worktree {
            return Err(unavailable("git reported no new stash identity but changed the worktree"));
        }
        return Ok(GitStashPushOutcome::NothingToStash);
    }
    let identity = String::from_utf8_lossy(&after.stash).trim().to_owned();
    if identity.is_empty() {
        return Err(unavailable("git created a stash without a commit identity"));
    }
    CommitHash::try_new(identity)
        .map(GitStashPushOutcome::Created)
        .map_err(|error| unavailable(format!("git created an invalid stash identity: {error}")))
}
fn verify_stash_identity(
    repo: &SystemGitRepo,
    expected: &CommitHash,
    entries: &StashListSummary,
) -> Result<usize, GitStashPopError> {
    match entries.expected_matches {
        1 => entries.expected_index.ok_or_else(|| {
            unavailable("git stash list matched the recorded identity without an index").into()
        }),
        0 if entries.count == 0 => {
            Err(GitStashPopError::StashIdentityMissing(expected.clone()))
        }
        0 => {
            let object_args =
                ["rev-parse", "--verify", "--quiet", &format!("{}^{{commit}}", expected)];
            let object = run_git(repo, &object_args)?;
            if object.status.success() {
                let actual = entries
                    .first_identity
                    .clone()
                    .ok_or_else(|| GitStashPopError::StashIdentityMissing(expected.clone()))?;
                Err(GitStashPopError::StashIdentityMismatch {
                    expected: expected.clone(),
                    actual,
                })
            } else if object.status.code() == Some(1) {
                Err(GitStashPopError::StashIdentityMissing(expected.clone()))
            } else {
                Err(command_failure(&object_args, &object).into())
            }
        }
        _ => Err(unavailable(format!(
            "recorded stash identity appears more than once; refusing to choose an entry: {expected}"
        ))
        .into()),
    }
}
fn drop_exact_stash(
    repo: &SystemGitRepo,
    expected: &CommitHash,
) -> Result<(Vec<u8>, usize), GitStashPopError> {
    let entries = stash_list_snapshot(repo, expected)?;
    let target_index = verify_stash_identity(repo, expected, &entries)?;
    let selector = format!("stash@{{{target_index}}}");
    let args = ["stash", "drop", "--quiet", selector.as_str()];
    let output = run_git_operation(repo, &args)?;
    if !output.status.success() {
        return Err(command_failure(&args, &output).into());
    }
    let remaining_count = entries
        .count
        .checked_sub(1)
        .ok_or_else(|| unavailable("git stash list count underflowed while dropping an entry"))?;
    Ok((entries.without_expected_digest, remaining_count))
}

fn handle_record_persistence_failure(
    repo: &SystemGitRepo,
    record: &StashRecord,
    record_failure: StashOperationError,
) -> Result<GitStashPushOutcome, GitStashPushError> {
    match &record.outcome {
        GitStashPushOutcome::NothingToStash => Err(record_failure.into()),
        GitStashPushOutcome::Created(expected) => {
            let state_dir = stash_state_dir(repo)?;
            match stash_record::read(&state_dir) {
                Ok(Some(persisted)) if persisted == *record => Err(unavailable(format!(
                    "guarded stash pairing record persistence was not confirmed, but the matching record remains and the stash was retained for recovery: {record_failure}"
                ))
                .into()),
                Ok(None) => match restore_exact_stash(repo, expected) {
                    Ok(()) => Err(unavailable(format!(
                        "guarded stash pairing record could not be persisted; the stash was restored: {record_failure}"
                    ))
                    .into()),
                    Err(rollback_failure) => Err(unavailable(format!(
                        "guarded stash pairing record could not be persisted and stash rollback failed; recorded identity {expected} requires manual recovery ({record_failure}; {rollback_failure})"
                    ))
                    .into()),
                },
                Ok(Some(_)) => Err(unavailable(format!(
                    "guarded stash pairing record persistence failed and an unexpected record remains; recorded identity {expected} requires manual recovery ({record_failure})"
                ))
                .into()),
                Err(record_read_failure) => Err(unavailable(format!(
                    "guarded stash pairing record persistence failed and its state could not be inspected; recorded identity {expected} requires manual recovery ({record_failure}; {record_read_failure})"
                ))
                .into()),
            }
        }
    }
}

fn restore_exact_stash(
    repo: &SystemGitRepo,
    expected: &CommitHash,
) -> Result<(), GitStashPopError> {
    let before = guarded_stash_snapshot(repo)?;
    // Re-verify the recorded OID immediately before applying it; the apply
    // operation itself is addressed by immutable identity, never by index.
    let entries_before_apply = stash_list_snapshot(repo, expected)?;
    let _ = verify_stash_identity(repo, expected, &entries_before_apply)?;
    let apply_args = ["stash", "apply", expected.as_ref()];
    let output = match run_git_operation(repo, &apply_args) {
        Ok(output) => output,
        Err(error) => {
            return Err(re_adjudicate_after_uncertain_operation(
                repo,
                &before,
                &apply_args,
                &error,
            )
            .into());
        }
    };
    let after = match guarded_stash_snapshot(repo) {
        Ok(after) => after,
        Err(error) => {
            return Err(re_adjudicate_after_uncertain_operation(
                repo,
                &before,
                &apply_args,
                &error,
            )
            .into());
        }
    };
    if before.branch_refs != after.branch_refs {
        return Err(GitStashPopError::ForbiddenBranchRefUpdate);
    }
    if !output.status.success() {
        return Err(command_failure(&apply_args, &output).into());
    }
    // Resolve the exact reflog entry from the recorded OID immediately before
    // removal. Git's supported stash-drop transaction updates the ref and its
    // reflog together; the adapter lock and reference-transaction hook provide
    // the guarded-path serialization described by the stash ADR.
    let expected_remaining = drop_exact_stash(repo, expected)?;
    let after_entries = stash_list_snapshot(repo, expected)?;
    if after_entries.expected_matches != 0
        || after_entries.count != expected_remaining.1
        || after_entries.digest != expected_remaining.0
    {
        return Err(unavailable(
            "exact guarded stash removal changed an unexpected stash entry; pending record retained",
        )
        .into());
    }
    Ok(())
}
fn re_adjudicate_after_uncertain_operation(
    repo: &SystemGitRepo,
    before: &GuardedStashSnapshot,
    operation: &[&str],
    reason: &StashOperationError,
) -> StashOperationError {
    match guarded_stash_snapshot(repo) {
        Ok(after) if before.branch_refs != after.branch_refs => {
            StashOperationError::ForbiddenBranchRefUpdate
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
    fn push(&self) -> Result<GitStashPushOutcome, GitStashPushError> {
        let repo = SystemGitRepo::discover()
            .map_err(|error| unavailable(format!("cannot discover git repository: {error}")))?;
        let _lock = stash_lock(&repo)?;
        let state_dir = stash_state_dir(&repo)?;
        if stash_record::read(&state_dir).map_err(record_error)?.is_some() {
            return Err(GitStashPushError::PendingGuardedStashExists);
        }
        let origin = worktree_identity(&repo)?;
        let before = guarded_stash_snapshot(&repo)?;
        // `--include-untracked` is the long form of the ADR's `-u`
        // requirement and captures untracked track artifacts.
        let args: &[&str] = &["stash", "push", "--include-untracked"];
        let output = match run_git_operation(&repo, args) {
            Ok(output) => output,
            Err(error) => {
                return Err(
                    re_adjudicate_after_uncertain_operation(&repo, &before, args, &error).into()
                );
            }
        };
        let after = match guarded_stash_snapshot(&repo) {
            Ok(after) => after,
            Err(error) => {
                return Err(
                    re_adjudicate_after_uncertain_operation(&repo, &before, args, &error).into()
                );
            }
        };
        if before.branch_refs != after.branch_refs {
            return Err(StashOperationError::ForbiddenBranchRefUpdate.into());
        }
        if !output.status.success() {
            return Err(command_failure(args, &output).into());
        }
        // Keep the order stash push -> OID capture -> record persistence. A
        // crash in that window intentionally enters AC-13's absent-record
        // recovery lane; no transactional machinery is added here.
        let outcome = push_outcome(&before, &after)?;
        let record = StashRecord::new(outcome, origin);
        match persist_record(&repo, &record) {
            Ok(()) => Ok(record.outcome.clone()),
            Err(record_failure) => {
                handle_record_persistence_failure(&repo, &record, record_failure)
            }
        }
    }
    fn pop(&self) -> Result<(), GitStashPopError> {
        let repo = SystemGitRepo::discover()
            .map_err(|error| unavailable(format!("cannot discover git repository: {error}")))?;
        let _lock = stash_lock(&repo)?;
        let persisted = pending_record(&repo)?;
        let current = worktree_identity(&repo)?;
        if persisted.worktree != current {
            return Err(unavailable(
                "guarded stash pairing belongs to a different linked worktree; refusing to apply or clear it",
            )
            .into());
        }
        match &persisted.outcome {
            GitStashPushOutcome::NothingToStash => clear_record(&repo, &persisted),
            GitStashPushOutcome::Created(expected) => {
                restore_exact_stash(&repo, expected)?;
                clear_record(&repo, &persisted)
            }
        }
    }
}
#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::{
        FsGitStashAdapter, MAX_STASH_OUTPUT_BYTES, guarded_stash_snapshot,
        re_adjudicate_after_uncertain_operation,
    };
    use crate::git_cli::SystemGitRepo;
    use crate::git_cli::stash_record::{self, STASH_RECORD_FILE, StashRecord};
    use domain::CommitHash;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::{Mutex, OnceLock};
    use usecase::git_stash::{
        GitStashPopError, GitStashPort, GitStashPushError, GitStashPushOutcome,
    };
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
    fn shared_git_dir(root: &Path) -> PathBuf {
        PathBuf::from(
            output(root, &["rev-parse", "--path-format=absolute", "--git-common-dir"]).trim(),
        )
    }
    fn record_path(root: &Path) -> PathBuf {
        shared_git_dir(root).join(STASH_RECORD_FILE)
    }
    fn stash_identities(root: &Path) -> Vec<String> {
        output(root, &["stash", "list", "--format=%H"]).lines().map(str::to_owned).collect()
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
        adapter.push().expect("stash push must succeed");
        assert!(!repo.path().join("untracked.txt").exists());
        assert_eq!(output(repo.path(), &["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "main");
        adapter.pop().expect("stash pop must succeed");
        assert!(!record_path(repo.path()).exists());
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
    fn test_fs_git_stash_adapter_clean_push_and_pop_preserve_unrelated_stash() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "unrelated change\n")
            .expect("unrelated stash change must be written");
        run_git(repo.path(), &["stash", "push", "--include-untracked", "-m", "unrelated"]);
        let unrelated = stash_identities(repo.path());
        assert_eq!(unrelated.len(), 1);
        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();
        let outcome = adapter.push().expect("clean guarded push must succeed");
        assert_eq!(outcome, GitStashPushOutcome::NothingToStash);
        assert!(record_path(repo.path()).is_file());
        adapter.pop().expect("paired clean pop must be a no-op");
        assert_eq!(stash_identities(repo.path()), unrelated);
        assert!(!record_path(repo.path()).exists());
    }

    #[test]
    fn test_fs_git_stash_adapter_pairing_record_refuses_cross_worktree_pop() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        let linked = tempfile::tempdir().expect("linked worktree fixture must exist");
        let linked_path = linked.path().to_str().expect("linked worktree path must be UTF-8");
        run_git(repo.path(), &["worktree", "add", "-q", "-b", "linked", linked_path, "HEAD"]);

        fs::write(repo.path().join("tracked.txt"), "saved\n")
            .expect("guarded change must be written");
        let _main_cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();
        adapter.push().expect("guarded push must succeed");
        assert!(record_path(repo.path()).is_file());
        assert!(!repo.path().join(STASH_RECORD_FILE).exists());

        {
            let _linked_cwd = CurrentDirGuard::enter(linked.path());
            let error = adapter
                .pop()
                .expect_err("a different linked worktree must not consume the pairing");
            assert!(matches!(error, GitStashPopError::Unavailable(_)));
            assert!(record_path(repo.path()).is_file());
        }
        adapter.pop().expect("the originating worktree must restore the pairing");
        run_git(repo.path(), &["worktree", "remove", "--force", linked_path]);
        assert!(!record_path(repo.path()).exists());
        assert_eq!(
            fs::read_to_string(repo.path().join("tracked.txt"))
                .expect("main worktree file must remain readable"),
            "saved\n"
        );
    }

    #[test]
    fn test_git_stash_pop_oid_targets_recorded_entry_after_stack_insert() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        fs::write(repo.path().join(".gitignore"), format!("/{STASH_RECORD_FILE}\n"))
            .expect("guarded stash record ignore rule must be written");
        run_git(repo.path(), &["add", ".gitignore"]);
        run_git(repo.path(), &["commit", "-m", "ignore guarded stash record"]);
        let before = branch_state(repo.path());
        fs::write(repo.path().join("tracked.txt"), "guarded\n")
            .expect("guarded change must be written");
        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();
        let outcome = adapter.push().expect("guarded stash push must succeed");
        let expected_commit = match &outcome {
            GitStashPushOutcome::Created(commit) => commit.clone(),
            GitStashPushOutcome::NothingToStash => panic!("fixture must create a guarded stash"),
        };
        let stored = stash_record::read(&shared_git_dir(repo.path()))
            .expect("guarded stash record must be readable")
            .map(|record| record.outcome);
        assert_eq!(
            stored,
            Some(GitStashPushOutcome::Created(expected_commit)),
            "record must persist the commit created by the guarded push"
        );
        let ignored = Command::new("git")
            .args(["check-ignore", "--quiet", "--", STASH_RECORD_FILE])
            .current_dir(repo.path())
            .status()
            .expect("git check-ignore must spawn");
        assert!(ignored.success(), "guarded stash record must be gitignored");
        fs::write(repo.path().join("tracked.txt"), "unrelated\n")
            .expect("unrelated change must be written");
        run_git(repo.path(), &["stash", "push", "--include-untracked", "-m", "unrelated"]);
        let unrelated = output(repo.path(), &["rev-parse", "refs/stash"]).trim().to_owned();
        adapter.pop().expect("recorded stash pop must succeed");
        assert_eq!(
            fs::read_to_string(repo.path().join("tracked.txt")).expect("restored file exists"),
            "guarded\n"
        );
        assert_eq!(
            stash_identities(repo.path()),
            vec![unrelated],
            "stash reflog after exact delete: {}",
            fs::read_to_string(repo.path().join(".git/logs/refs/stash"))
                .expect("stash reflog must be readable")
        );
        let reflog = fs::read_to_string(repo.path().join(".git/logs/refs/stash"))
            .expect("stash reflog must remain readable");
        let zero_oid = "0".repeat(40);
        assert_eq!(
            reflog.split_whitespace().next(),
            Some(zero_oid.as_str()),
            "dropping a middle stash must rewrite the surviving reflog predecessor"
        );
        assert_eq!(branch_state(repo.path()), before);
        assert!(!record_path(repo.path()).exists());
    }
    #[test]
    fn test_fs_git_stash_adapter_second_push_while_record_exists_fails_closed() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "first change\n")
            .expect("first change must be written");
        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();
        adapter.push().expect("first guarded push must succeed");
        fs::write(repo.path().join("tracked.txt"), "second change\n")
            .expect("second change must be written");
        let error = adapter.push().expect_err("pending pairing must block a second push");
        assert!(matches!(error, GitStashPushError::PendingGuardedStashExists));
        assert_eq!(stash_identities(repo.path()).len(), 1);
        fs::write(repo.path().join("tracked.txt"), "base\n")
            .expect("fixture must be cleaned before paired pop");
        adapter.pop().expect("first pairing must remain recoverable");
    }
    #[test]
    fn test_fs_git_stash_adapter_missing_identity_fails_closed_and_retains_record() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "saved\n")
            .expect("stash change must be written");
        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();
        let outcome = adapter.push().expect("guarded push must succeed");
        let expected_identity = match &outcome {
            GitStashPushOutcome::Created(identity) => identity.clone(),
            GitStashPushOutcome::NothingToStash => panic!("fixture must create a stash"),
        };
        // Simulate an unguarded/operator action outside the guarded threat model.
        run_git(repo.path(), &["stash", "drop", "stash@{0}"]);
        let error = adapter.pop().expect_err("a dropped recorded stash must fail closed");
        assert!(matches!(
            error,
            GitStashPopError::StashIdentityMissing(identity) if identity == expected_identity
        ));
        assert!(record_path(repo.path()).is_file());
    }
    #[test]
    fn test_fs_git_stash_adapter_mismatched_identity_fails_closed_and_retains_record() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        let expected = CommitHash::try_new(output(repo.path(), &["rev-parse", "HEAD"]).trim())
            .expect("HEAD must be a valid commit identity");
        fs::write(repo.path().join("tracked.txt"), "unrelated\n")
            .expect("unrelated stash change must be written");
        run_git(repo.path(), &["stash", "push", "--include-untracked", "-m", "unrelated"]);
        let actual = CommitHash::try_new(output(repo.path(), &["rev-parse", "refs/stash"]).trim())
            .expect("stash identity must be valid");
        let outcome = GitStashPushOutcome::Created(expected.clone());
        let _cwd = CurrentDirGuard::enter(repo.path());
        let repository = SystemGitRepo::discover().expect("repository must be discoverable");
        let state_dir = super::stash_state_dir(&repository).expect("state dir must be readable");
        let worktree =
            super::worktree_identity(&repository).expect("worktree identity must be readable");
        stash_record::write(&state_dir, &StashRecord::new(outcome, worktree))
            .expect("mismatch record must be written");
        let error =
            FsGitStashAdapter::new().pop().expect_err("mismatched identity must fail closed");
        assert!(matches!(
            error,
            GitStashPopError::StashIdentityMismatch { expected: found_expected, actual: found_actual }
                if found_expected == expected && found_actual == actual
        ));
        assert_eq!(stash_identities(repo.path()), vec![actual.to_string()]);
        assert!(record_path(repo.path()).is_file());
    }
    #[test]
    fn test_fs_git_stash_adapter_pop_failure_fails_closed_and_retains_record() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "saved\n")
            .expect("stash change must be written");
        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();
        adapter.push().expect("guarded push must succeed");
        fs::write(repo.path().join("tracked.txt"), "conflicting local change\n")
            .expect("conflicting local change must be written");

        let error = adapter.pop().expect_err("a conflicting stash pop must fail closed");
        assert!(matches!(error, GitStashPopError::Unavailable(_)));
        assert_eq!(stash_identities(repo.path()).len(), 1);
        assert!(record_path(repo.path()).is_file());
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
        adapter.push().expect("large stash push must succeed");
        assert!(!first_artifact.exists());
        adapter.pop().expect("large stash pop must succeed");
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
        adapter.push().expect("push must succeed with many local heads");
        adapter.pop().expect("pop must succeed with many local heads");
        assert_eq!(
            fs::read_to_string(repo.path().join("untracked.txt"))
                .expect("untracked file must be restored"),
            "saved\n"
        );
    }
    #[test]
    fn test_fs_git_stash_adapter_pop_without_saved_worktree_returns_no_pending_guarded_stash() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        let _cwd = CurrentDirGuard::enter(repo.path());
        let error =
            FsGitStashAdapter::new().pop().expect_err("pop without a saved stash must fail closed");
        assert!(matches!(&error, GitStashPopError::NoPendingGuardedStash));
        let guidance = error.to_string();
        assert!(guidance.contains("no pending guarded stash record"));
        assert!(guidance.contains("git stash list"));
        assert!(guidance.contains("expected entry or OID"));
        assert!(guidance.contains("bin/sotp git stash push"));
        assert!(guidance.contains("recover the orphaned stash"));
    }
    #[test]
    fn test_fs_git_stash_adapter_crash_window_absent_record_leaves_orphaned_stash_untouched() {
        let _lock = cwd_lock().lock().expect("CWD lock must not be poisoned");
        let repo = init_repo();
        fs::write(repo.path().join("tracked.txt"), "saved\n")
            .expect("stash change must be written");
        let _cwd = CurrentDirGuard::enter(repo.path());
        let adapter = FsGitStashAdapter::new();
        let outcome = adapter.push().expect("guarded stash push must succeed");
        let expected_identity = match &outcome {
            GitStashPushOutcome::Created(identity) => identity.to_string(),
            GitStashPushOutcome::NothingToStash => panic!("fixture must create a guarded stash"),
        };
        let orphaned_stashes = stash_identities(repo.path());
        assert!(
            orphaned_stashes.iter().any(|identity| identity == &expected_identity),
            "guarded stash identity must be present in git stash list"
        );
        assert!(record_path(repo.path()).is_file());

        // Simulate the ADR's crash window: the guarded push has created the
        // stash and persisted its record, then deleting the record recreates
        // the externally visible state of a crash before record persistence.
        fs::remove_file(record_path(repo.path()))
            .expect("crash-window fixture must remove the pairing record");

        let error = adapter.pop().expect_err("an absent crash-window record must fail closed");
        assert!(matches!(&error, GitStashPopError::NoPendingGuardedStash));
        let guidance = error.to_string();
        assert!(guidance.contains("no pending guarded stash record"));
        assert!(guidance.contains("git stash list"));
        assert!(guidance.contains("expected entry or OID"));
        assert!(guidance.contains("bin/sotp git stash push"));
        assert!(guidance.contains("recover the orphaned stash"));
        assert_eq!(stash_identities(repo.path()), orphaned_stashes);
        assert!(!record_path(repo.path()).exists());
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
        adapter.push().expect("stash push must succeed");
        assert_eq!(branch_state(repo.path()), before);
        adapter.pop().expect("stash pop must succeed");
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
        assert!(matches!(error, super::StashOperationError::Unavailable(_)));
        assert!(error.to_string().contains("stash or worktree state changed"));
    }
}
