//! Filesystem and git adapters for guarded base merges, plus their persistence codec.

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

use domain::branch_strategy::{BaseBranchName, BaseMergeDirection, derive_base_merge_direction};
use domain::tddd::catalogue_v2::RustdocBaselineCapturePort;
use domain::{CommitHash, TrackBranch, TrackId};
use fs4::fs_std::FileExt as _;
use usecase::base_merge::{
    BaseMergeAttemptOutcome, BaseMergeCleanupPort, BaseMergeCleanupRequest, BaseMergeContextError,
    BaseMergeContextPort, BaseMergeGitError, BaseMergeGitPort, BaselineReplacementError,
    SyncBaseRecordError, ViewsRegenerationError,
};
use usecase::git_workflow::DiagnosticText;

use crate::git_cli::{
    SystemGitRepo, collect_bounded_git_output, guarded_git_command, isolated_bounded_git_output,
    spawn_bounded_git_child, without_history_rewrites, without_repository_selection,
};
use crate::tddd::rustdoc_baseline_capture_adapter::RustdocBaselineCaptureAdapter;
use crate::track::render::sync_rendered_views;
use crate::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};

const MAX_BASE_MERGE_GIT_OUTPUT_BYTES: usize = 8 * 1024;
const MAX_CLEANUP_TREE_DEPTH: usize = 64;
const MAX_CLEANUP_TREE_ENTRIES: usize = 10_000;
const MAX_CLEANUP_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CLEANUP_TREE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_SYNC_BASE_RECORD_BYTES: u64 = 64 * 1024;
pub(super) const BASELINE_REPLACEMENT_PHASE_MARKER: &str = ".sotp-baseline-replacement-phase";
pub(super) const TRACK_WRITER_LOCK_FILE: &str = "metadata.json.lock";

mod cleanup_tree;
mod merge_state_probe;
mod publication;
mod sync_base;
mod sync_base_record;

use merge_state_probe::{
    base_commit_is_merged_into_head, has_unmerged_paths, merge_head_is_present,
    merge_head_matches_commit,
};

use cleanup_tree::{
    capture_baselines_in_worktree, collect_validated_baselines, copy_cleanup_inputs_with_baselines,
    copy_tree_with_baselines, remove_tree_bounded,
};
#[cfg(test)]
use cleanup_tree::{copy_cleanup_inputs, replace_tree};
#[cfg(test)]
use publication::acquire_track_writer_lock;
use publication::{
    PendingWriterLock, generated_baseline_file_names, path_exists, promote_baseline_recovery_slot,
    publish_baseline_replacements, reconcile_interrupted_replacement, with_writer_lock,
    write_replacement_phase_marker,
};
use sync_base::{read_regular_file_bounded, write_sync_base_record_atomically};
pub use sync_base_record::{SyncBaseRecord, SyncBaseRecordSchemaVersion};
#[cfg(test)]
pub(crate) use sync_base_record::{decode, encode};

/// Filesystem-backed loader for the authoritative active-track merge direction.
pub struct FsBaseMergeContextAdapter;

#[allow(clippy::new_without_default)]
impl FsBaseMergeContextAdapter {
    /// Creates the filesystem-backed context adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl BaseMergeContextPort for FsBaseMergeContextAdapter {
    fn load_direction(
        &self,
        workspace_root: &Path,
    ) -> Result<BaseMergeDirection, BaseMergeContextError> {
        let repository_root =
            resolve_workspace_repository_root(workspace_root).map_err(context_unavailable)?;
        let current = read_current_track_branch(&repository_root).map_err(context_unavailable)?;
        let direction = load_authoritative_direction(&repository_root, &current)
            .map_err(context_unavailable)?;

        if current != *direction.active_track() {
            return Err(BaseMergeContextError::ActiveTrackMismatch {
                current,
                expected: direction.active_track().clone(),
            });
        }

        Ok(direction)
    }
}

/// Guarded Git implementation of a base-to-track merge.
pub struct FsBaseMergeGitAdapter;

#[allow(clippy::new_without_default)]
impl FsBaseMergeGitAdapter {
    /// Creates the guarded Git adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl BaseMergeGitPort for FsBaseMergeGitAdapter {
    fn merge_base(
        &self,
        workspace_root: &Path,
        direction: &BaseMergeDirection,
    ) -> Result<BaseMergeAttemptOutcome, BaseMergeGitError> {
        let repository_root =
            resolve_workspace_repository_root(workspace_root).map_err(git_execution_error)?;
        let locked_branch =
            read_current_track_branch(&repository_root).map_err(git_execution_error)?;
        let track_id = track_id_from_branch(&locked_branch).map_err(git_execution_error)?;
        let _merge_lock = acquire_base_merge_lock(&repository_root, &track_id)?;
        let current = read_current_track_branch(&repository_root).map_err(git_execution_error)?;
        if current != locked_branch {
            return Err(git_execution_error(
                "active track branch changed while acquiring merge lock",
            ));
        }
        let authoritative_direction = load_authoritative_direction(&repository_root, &current)
            .map_err(git_execution_error)?;
        if current != *authoritative_direction.active_track() {
            return Err(git_execution_error("active track branch changed before merge"));
        }
        if direction != &authoritative_direction {
            return Err(git_execution_error(
                "supplied merge direction differs from the active track snapshot",
            ));
        }

        // Only a conflict created by this guarded merge may authorize the
        // recovery workflow: refuse to run over pre-existing merge state, so
        // an interrupted earlier merge or unrelated unmerged entries are never
        // adjudicated as this operation's Conflicted outcome.
        if merge_head_is_present(&repository_root)? {
            return Err(git_execution_error("a merge is already in progress"));
        }
        if has_unmerged_paths(&repository_root)? {
            return Err(git_execution_error("worktree has pre-existing unmerged paths"));
        }

        let base_commit = resolve_base_commit(&repository_root, authoritative_direction.source())?;
        let output = match run_guarded_merge(&repository_root, &base_commit) {
            Ok(output) => output,
            Err(_) => return adjudicate_merge_after_runner_error(&repository_root, base_commit),
        };
        if output.status.success() {
            if merge_head_is_present(&repository_root)? || has_unmerged_paths(&repository_root)? {
                return Err(git_execution_error(
                    "guarded git merge reported success with unresolved merge state",
                ));
            }
            return Ok(BaseMergeAttemptOutcome::Clean { base_commit });
        }

        if output.status.code() == Some(1) && has_unmerged_paths(&repository_root)? {
            if merge_head_matches_commit(&repository_root, &base_commit)? {
                return Ok(BaseMergeAttemptOutcome::Conflicted);
            }
            return Err(git_execution_error("guarded git merge left unrelated unmerged paths"));
        }

        Err(git_execution_error("guarded git merge failed"))
    }
}

/// Filesystem-backed implementation of the ordered clean-merge cleanup.
pub struct FsBaseMergeCleanupAdapter {
    baseline_capture: Arc<dyn RustdocBaselineCapturePort>,
    pending_writer_lock: Mutex<Option<PendingWriterLock>>,
}

#[allow(clippy::new_without_default)]
impl FsBaseMergeCleanupAdapter {
    /// Creates the filesystem-backed cleanup adapter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            baseline_capture: Arc::new(RustdocBaselineCaptureAdapter::new()),
            pending_writer_lock: Mutex::new(None),
        }
    }

    #[cfg(test)]
    fn with_baseline_capture(capture: Arc<dyn RustdocBaselineCapturePort>) -> Self {
        Self { baseline_capture: capture, pending_writer_lock: Mutex::new(None) }
    }
}

impl BaseMergeCleanupPort for FsBaseMergeCleanupAdapter {
    fn regenerate_views(
        &self,
        request: &BaseMergeCleanupRequest,
    ) -> Result<(), ViewsRegenerationError> {
        sync_rendered_views(&request.workspace_root, Some(request.track_id.as_ref()))
            .map(|_| ())
            .map_err(|error| {
                ViewsRegenerationError::Regeneration(DiagnosticText::new(error.to_string()))
            })
    }

    fn replace_baselines(
        &self,
        request: &BaseMergeCleanupRequest,
    ) -> Result<(), BaselineReplacementError> {
        with_writer_lock(
            &self.pending_writer_lock,
            request,
            true,
            || replace_baselines_from_exact_commit(request, Arc::clone(&self.baseline_capture)),
            |error| BaselineReplacementError::Publish(DiagnosticText::new(error)),
        )
    }

    fn write_sync_base_record(
        &self,
        request: &BaseMergeCleanupRequest,
    ) -> Result<(), SyncBaseRecordError> {
        with_writer_lock(
            &self.pending_writer_lock,
            request,
            false,
            || write_sync_base_record_atomically(request),
            |error| SyncBaseRecordError::Write(DiagnosticText::new(error)),
        )
    }
}

fn replace_baselines_from_exact_commit(
    request: &BaseMergeCleanupRequest,
    baseline_capture: Arc<dyn RustdocBaselineCapturePort>,
) -> Result<(), BaselineReplacementError> {
    let repository_root = resolve_workspace_repository_root(&request.workspace_root)
        .map_err(|detail| BaselineReplacementError::Isolation(DiagnosticText::new(detail)))?;
    let track_dir = request.workspace_root.join("track/items").join(request.track_id.as_ref());
    let items_dir = request.workspace_root.join("track/items");
    reject_symlinks_up_to_root(&items_dir).map_err(|error| {
        BaselineReplacementError::Isolation(DiagnosticText::new(error.to_string()))
    })?;
    reject_symlinks_below(&track_dir, &items_dir).map_err(|error| {
        BaselineReplacementError::Isolation(DiagnosticText::new(error.to_string()))
    })?;
    if !track_dir.is_dir() {
        return Err(BaselineReplacementError::Isolation(DiagnosticText::new(
            "active track directory is unavailable",
        )));
    }
    let generated_baseline_files = generated_baseline_file_names(&request.workspace_root)
        .map_err(|error| BaselineReplacementError::Isolation(DiagnosticText::new(error)))?;

    let track_parent = track_dir.parent().ok_or_else(|| {
        BaselineReplacementError::Isolation(DiagnosticText::new(
            "active track directory has no parent directory",
        ))
    })?;
    let track_root = track_parent.parent().ok_or_else(|| {
        BaselineReplacementError::Isolation(DiagnosticText::new(
            "track items directory has no recovery root",
        ))
    })?;
    reject_symlinks_up_to_root(track_root).map_err(|error| {
        BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot inspect track recovery root: {error}"
        )))
    })?;
    let recovery_root = track_root.join(".sotp-baseline-recovery");
    fs::create_dir_all(&recovery_root).map_err(|error| {
        BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot create baseline recovery root: {error}"
        )))
    })?;
    fs::File::open(track_root).and_then(|directory| directory.sync_all()).map_err(|error| {
        BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot persist baseline recovery root entry: {error}"
        )))
    })?;
    reject_symlinks_below(&recovery_root, track_root).map_err(|error| {
        BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot inspect baseline recovery root: {error}"
        )))
    })?;
    let recovery_slot = recovery_root.join(request.track_id.as_ref());
    let replacement =
        recovery_root.join(format!(".sotp-baseline-replacement-{}", request.track_id));
    let active_phase_marker = track_dir.join(BASELINE_REPLACEMENT_PHASE_MARKER);
    reject_symlinks_below(&active_phase_marker, &items_dir).map_err(|error| {
        BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot inspect active baseline replacement phase marker: {error}"
        )))
    })?;
    reject_symlinks_below(&replacement, &recovery_root).map_err(|error| {
        BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot inspect baseline replacement staging slot: {error}"
        )))
    })?;
    let had_replacement = path_exists(&replacement)
        .map_err(|error| BaselineReplacementError::Isolation(DiagnosticText::new(error)))?;
    if had_replacement {
        reconcile_interrupted_replacement(
            &replacement,
            &recovery_slot,
            &recovery_root,
            &track_dir,
            &generated_baseline_files,
        )
        .map_err(|error| BaselineReplacementError::Isolation(DiagnosticText::new(error)))?;
    }
    if path_exists(&active_phase_marker)
        .map_err(|error| BaselineReplacementError::Isolation(DiagnosticText::new(error)))?
    {
        let recovered_canonical_slot = path_exists(&recovery_slot)
            .map_err(|error| BaselineReplacementError::Isolation(DiagnosticText::new(error)))?;
        if !had_replacement && !recovered_canonical_slot {
            return Err(BaselineReplacementError::Isolation(DiagnosticText::new(
                "active track contains an unreconciled baseline replacement phase marker",
            )));
        }
        fs::remove_file(&active_phase_marker).map_err(|error| {
            BaselineReplacementError::Isolation(DiagnosticText::new(format!(
                "cannot clear recovered baseline replacement phase marker: {error}"
            )))
        })?;
        fs::File::open(&track_dir).and_then(|directory| directory.sync_all()).map_err(|error| {
            BaselineReplacementError::Isolation(DiagnosticText::new(format!(
                "cannot persist recovered baseline replacement phase marker removal: {error}"
            )))
        })?;
    }
    fs::create_dir(&replacement).map_err(|error| {
        BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot create baseline replacement staging slot: {error}"
        )))
    })?;

    let worktree = match create_unique_directory(&repository_root, ".sotp-base-merge-worktree-") {
        Ok(worktree) => worktree,
        Err(error) => {
            let detail =
                match cleanup_directory(&replacement, "baseline replacement staging directory") {
                    Ok(()) => error.to_string(),
                    Err(cleanup) => format!("{error}; {cleanup}"),
                };
            return Err(BaselineReplacementError::Isolation(DiagnosticText::new(detail)));
        }
    };
    let mut published = false;
    let mut exchanged = false;
    let result: Result<(), BaselineReplacementError> = (|| {
        add_commit_pinned_worktree(&repository_root, &worktree, &request.base_commit)
            .map_err(|error| BaselineReplacementError::Isolation(DiagnosticText::new(error)))?;
        copy_cleanup_inputs_with_baselines(
            &request.workspace_root,
            &worktree,
            request.track_id.as_ref(),
            &generated_baseline_files,
        )
        .map_err(|error| BaselineReplacementError::Isolation(DiagnosticText::new(error)))?;
        capture_baselines_in_worktree(
            &worktree,
            request.track_id.as_ref(),
            Arc::clone(&baseline_capture),
        )?;
        collect_validated_baselines(&worktree, request.track_id.as_ref())
            .map_err(|error| BaselineReplacementError::Validation(DiagnosticText::new(error)))?;
        write_replacement_phase_marker(&replacement)?;
        copy_tree_with_baselines(
            &worktree.join("track/items").join(request.track_id.as_ref()),
            &replacement,
            true,
            &recovery_root,
            &generated_baseline_files,
        )
        .map_err(|error| BaselineReplacementError::Validation(DiagnosticText::new(error)))?;
        publish_baseline_replacements(
            &track_dir,
            &replacement,
            &generated_baseline_files,
            &mut exchanged,
        )?;
        published = true;
        promote_baseline_recovery_slot(&replacement, &recovery_slot, &recovery_root)
    })();

    let removal = remove_commit_pinned_worktree(&repository_root, &worktree)
        .map_err(|error| format!("cannot unregister detached cleanup worktree: {error}"));
    let directory_cleanup = cleanup_directory(&worktree, "detached cleanup worktree");
    // A publication error may have occurred only after the atomic exchange,
    // leaving `replacement` as the complete prior track for recovery. Keep
    // both trees when that happens; rolling back and deleting the replacement
    // could discard writes made through either tree during publication.
    let replacement_cleanup = if result.is_err()
        && !published
        && !exchanged
        && !matches!(&result, Err(BaselineReplacementError::Restoration { .. }))
    {
        cleanup_directory(&replacement, "baseline replacement staging directory")
    } else {
        Ok(())
    };
    combine_baseline_cleanup_result(result, [removal, directory_cleanup, replacement_cleanup])
}

fn cleanup_directory(path: &Path, label: &str) -> Result<(), String> {
    let trusted_root = path
        .parent()
        .ok_or_else(|| format!("cannot remove {label} {}: no trusted parent", path.display()))?;
    remove_tree_bounded(path, trusted_root)
        .map_err(|error| format!("cannot remove {label} {}: {error}", path.display()))
}

fn combine_baseline_cleanup_result(
    result: Result<(), BaselineReplacementError>,
    cleanup_results: [Result<(), String>; 3],
) -> Result<(), BaselineReplacementError> {
    let cleanup_details: Vec<String> =
        cleanup_results.into_iter().filter_map(Result::err).collect();
    match (result, cleanup_details.is_empty()) {
        (Ok(()), true) => Ok(()),
        (Ok(()), false) => Err(BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cleanup after baseline replacement failed: {}",
            cleanup_details.join("; ")
        )))),
        (Err(error), true) => Err(error),
        (Err(error), false) => Err(append_cleanup_failure(error, cleanup_details.join("; "))),
    }
}

fn append_cleanup_failure(
    error: BaselineReplacementError,
    cleanup: String,
) -> BaselineReplacementError {
    let append = |detail: DiagnosticText| {
        DiagnosticText::new(format!("{detail}; cleanup also failed: {cleanup}"))
    };
    match error {
        BaselineReplacementError::Isolation(detail) => {
            BaselineReplacementError::Isolation(append(detail))
        }
        BaselineReplacementError::Generation(detail) => {
            BaselineReplacementError::Generation(append(detail))
        }
        BaselineReplacementError::Validation(detail) => {
            BaselineReplacementError::Validation(append(detail))
        }
        BaselineReplacementError::Publish(detail) => {
            BaselineReplacementError::Publish(append(detail))
        }
        BaselineReplacementError::Restoration { publish, restoration } => {
            BaselineReplacementError::Restoration { publish, restoration: append(restoration) }
        }
    }
}

fn create_unique_directory(parent: &Path, prefix: &str) -> std::io::Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| std::io::Error::other(error.to_string()))?
        .as_nanos();
    for suffix in 0..100_u32 {
        let path = parent.join(format!("{prefix}{}-{suffix}", std::process::id() ^ stamp as u32));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a unique cleanup directory",
    ))
}

fn add_commit_pinned_worktree(
    repository_root: &Path,
    worktree: &Path,
    base_commit: &CommitHash,
) -> Result<(), String> {
    let mut command = guarded_git_command();
    command
        .args(["worktree", "add", "--detach", "--"])
        .arg(worktree)
        .arg(base_commit.as_ref())
        .current_dir(repository_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    without_repository_selection(&mut command);
    without_history_rewrites(&mut command);
    let output = collect_bounded_git_output(
        spawn_bounded_git_child(&mut command).map_err(|error| error.to_string())?,
        MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    )
    .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("git worktree add failed: {}", String::from_utf8_lossy(&output.stderr).trim()))
    }
}

fn remove_commit_pinned_worktree(repository_root: &Path, worktree: &Path) -> Result<(), String> {
    let mut command = guarded_git_command();
    command
        .args(["worktree", "remove", "--force", "--"])
        .arg(worktree)
        .current_dir(repository_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    without_repository_selection(&mut command);
    without_history_rewrites(&mut command);
    let output = collect_bounded_git_output(
        spawn_bounded_git_child(&mut command).map_err(|error| error.to_string())?,
        MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    )
    .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git worktree remove failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
fn resolve_workspace_repository_root(workspace_root: &Path) -> Result<PathBuf, &'static str> {
    crate::track::symlink_guard::reject_symlinks_up_to_root(workspace_root)
        .map_err(|_| "workspace path is unavailable")?;
    let workspace = workspace_root.canonicalize().map_err(|_| "workspace path is unavailable")?;
    let repository = SystemGitRepo::discover_from_isolated(&workspace)
        .map_err(|_| "workspace is not a repository")?;
    let repository_root =
        repository.root().canonicalize().map_err(|_| "repository root is unavailable")?;
    if workspace != repository_root {
        return Err("workspace root does not match repository root");
    }
    Ok(repository_root)
}

fn read_current_track_branch(repository_root: &Path) -> Result<TrackBranch, &'static str> {
    let output = isolated_bounded_git_output(
        repository_root,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    )
    .map_err(|_| "current branch is unavailable")?;
    if !output.status.success() {
        return Err("current branch is unavailable");
    }
    let branch = std::str::from_utf8(&output.stdout).map_err(|_| "current branch is invalid")?;
    TrackBranch::try_new(branch.trim().to_owned())
        .map_err(|_| "current branch is not an active track")
}

fn track_id_from_branch(branch: &TrackBranch) -> Result<TrackId, &'static str> {
    let Some(track_id) = branch.as_ref().strip_prefix("track/") else {
        return Err("current branch is not an active track");
    };
    TrackId::try_new(track_id).map_err(|_| "current branch is not an active track")
}

fn load_authoritative_direction(
    repository_root: &Path,
    current: &TrackBranch,
) -> Result<BaseMergeDirection, &'static str> {
    let track_id = track_id_from_branch(current)?;
    let (metadata, _) = crate::track::fs_store::read_track_metadata(
        &repository_root.join("track/items"),
        &track_id,
    )
    .map_err(|_| "active track metadata is unavailable")?;
    derive_base_merge_direction(&metadata).map_err(|_| "active track direction is invalid")
}

fn resolve_base_commit(
    repository_root: &Path,
    source: &BaseBranchName,
) -> Result<CommitHash, BaseMergeGitError> {
    // Qualify as a heads ref: the documented ambiguous-ref precedence checks
    // refs/tags/<name> before refs/heads/<name>, so an unqualified name could
    // resolve a same-named tag instead of the snapshot branch.
    let revision = format!("refs/heads/{}^{{commit}}", source.as_str());
    let output = isolated_bounded_git_output(
        repository_root,
        &["rev-parse", "--verify", revision.as_str()],
        MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    )
    .map_err(|_| git_execution_error("base commit could not be resolved"))?;
    if !output.status.success() {
        return Err(git_execution_error("base commit could not be resolved"));
    }
    let resolved = std::str::from_utf8(&output.stdout)
        .map_err(|_| git_execution_error("base commit is invalid"))?;
    CommitHash::try_new(resolved.trim().to_owned())
        .map_err(|_| git_execution_error("base commit is invalid"))
}

fn run_guarded_merge(repository_root: &Path, base_commit: &CommitHash) -> std::io::Result<Output> {
    let mut command = guarded_git_command();
    command
        .args(["merge", "--no-ff", "--no-edit", "--"])
        .arg(base_commit.as_ref())
        .current_dir(repository_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    without_repository_selection(&mut command);
    without_history_rewrites(&mut command);
    let child = spawn_bounded_git_child(&mut command)?;
    collect_bounded_git_output(child, MAX_BASE_MERGE_GIT_OUTPUT_BYTES)
}

fn adjudicate_merge_after_runner_error(
    repository_root: &Path,
    base_commit: CommitHash,
) -> Result<BaseMergeAttemptOutcome, BaseMergeGitError> {
    if has_unmerged_paths(repository_root)? {
        if merge_head_matches_commit(repository_root, &base_commit)? {
            return Ok(BaseMergeAttemptOutcome::Conflicted);
        }
        return Err(git_execution_error(
            "guarded git merge runner failed with unresolved merge state",
        ));
    }
    if merge_head_is_present(repository_root)? {
        return Err(git_execution_error("guarded git merge ended with an unresolved merge state"));
    }
    if base_commit_is_merged_into_head(repository_root, &base_commit)? {
        return Ok(BaseMergeAttemptOutcome::Clean { base_commit });
    }
    Err(git_execution_error("guarded git merge could not be adjudicated after runner failure"))
}

fn acquire_base_merge_lock(
    repository_root: &Path,
    track_id: &TrackId,
) -> Result<fs::File, BaseMergeGitError> {
    let items_dir = repository_root.join("track/items");
    crate::track::symlink_guard::reject_symlinks_up_to_root(&items_dir)
        .map_err(|_| git_execution_error("base merge lock path is unavailable"))?;
    let track_dir = items_dir.join(track_id.as_ref());
    crate::track::symlink_guard::reject_symlinks_below(&track_dir, &items_dir)
        .map_err(|_| git_execution_error("base merge lock path is unavailable"))?;
    if !track_dir.is_dir() {
        return Err(git_execution_error("base merge lock path is unavailable"));
    }
    let lock_path = track_dir.join("metadata.json.lock");
    crate::track::symlink_guard::reject_symlinks_below(&lock_path, &items_dir)
        .map_err(|_| git_execution_error("base merge lock path is unavailable"))?;
    let lock_file = open_base_merge_lock_file(&lock_path)
        .map_err(|_| git_execution_error("base merge lock path is unavailable"))?;
    lock_file
        .try_lock_exclusive()
        .map_err(|_| git_execution_error("another guarded base merge is in progress"))?;
    Ok(lock_file)
}

fn open_base_merge_lock_file(lock_path: &Path) -> std::io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    #[cfg(windows)]
    options.custom_flags(0x0020_0000);
    #[cfg(not(any(unix, windows)))]
    return Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-follow lock open is unavailable on this platform",
    ));
    options.open(lock_path)
}

fn context_unavailable(detail: &'static str) -> BaseMergeContextError {
    BaseMergeContextError::Unavailable(DiagnosticText::new(detail))
}

fn git_execution_error(detail: &'static str) -> BaseMergeGitError {
    BaseMergeGitError::Execution(DiagnosticText::new(detail))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;
    use std::sync::Arc;

    use domain::{BranchStrategySnapshot, MergeMethod, NonEmptyString, TrackMetadata};

    fn git(root: &Path, args: &[&str]) {
        crate::verify::test_support::git_with_identity(root, args);
    }

    fn write_metadata(root: &Path, id: &str, base_branch: &str) {
        let track_dir = root.join("track/items").join(id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            format!(
                r#"{{
  "schema_version": 6,
  "id": "{id}",
  "branch": "track/{id}",
  "title": "Adapter fixture",
  "created_at": "2026-08-02T00:00:00Z",
  "updated_at": "2026-08-02T00:00:00Z",
  "branch_strategy_snapshot": {{
    "base_branch": "{base_branch}",
    "merge_target": "develop",
    "merge_method": "merge"
  }}
}}"#
            ),
        )
        .unwrap();
    }

    fn setup_repository(id: &str, base_branch: &str) -> tempfile::TempDir {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        git(root, &["init", "--quiet", "--initial-branch=develop"]);
        git(root, &["config", "user.email", "test@example.com"]);
        git(root, &["config", "user.name", "Base Merge Test"]);
        std::fs::write(root.join("shared.txt"), "initial\n").unwrap();
        git(root, &["add", "shared.txt"]);
        git(root, &["commit", "--quiet", "-m", "initial"]);
        git(root, &["switch", "--quiet", "-c", &format!("track/{id}")]);
        std::fs::write(root.join("track.txt"), "track\n").unwrap();
        git(root, &["add", "track.txt"]);
        git(root, &["commit", "--quiet", "-m", "track work"]);
        git(root, &["switch", "--quiet", "develop"]);
        std::fs::write(root.join("base.txt"), "base\n").unwrap();
        git(root, &["add", "base.txt"]);
        git(root, &["commit", "--quiet", "-m", "base work"]);
        git(root, &["switch", "--quiet", &format!("track/{id}")]);
        write_metadata(root, id, base_branch);
        fixture
    }

    fn setup_cleanup_repository() -> tempfile::TempDir {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        git(root, &["init", "--quiet", "--initial-branch=develop"]);
        git(root, &["config", "user.email", "test@example.com"]);
        git(root, &["config", "user.name", "Base Merge Test"]);
        std::fs::create_dir_all(root.join("libs/domain/src")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"libs/domain\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("libs/domain/Cargo.toml"),
            "[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(root.join("libs/domain/src/lib.rs"), "pub struct CleanupFixture;\n")
            .unwrap();
        std::fs::write(
            root.join("architecture-rules.json"),
            r#"{
  "version": 2,
  "module_limits": {"max_lines": 700, "warn_lines": 400, "exclude": []},
  "canonical_modules": [],
  "extra_dirs": [],
  "layers": [{
    "crate": "domain",
    "path": "libs/domain",
    "may_depend_on": [],
    "deny_reason": "",
    "verify": {"domain_purity": true, "domain_strings": true},
    "tddd": {
      "enabled": true,
      "catalogue_file": "domain-types.json",
      "schema_export": {"method": "rustdoc", "targets": ["domain"]}
    }
  }]
}"#,
        )
        .unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "--quiet", "-m", "minimal cleanup fixture"]);
        git(root, &["switch", "--quiet", "-c", "track/cleanup-test"]);
        write_metadata(root, "cleanup-test", "develop");
        let track_dir = root.join("track/items/cleanup-test");
        std::fs::write(
            track_dir.join("tddd-features.json"),
            r#"{"schema_version":1,"layers":{"domain":[]}}"#,
        )
        .unwrap();
        fixture
    }

    fn current_commit(root: &Path, revision: &str) -> String {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--verify", revision])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(output.status.success(), "revision must resolve: {revision}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn cleanup_direction() -> BaseMergeDirection {
        let track_id = TrackId::try_new("cleanup-test").unwrap();
        let branch = TrackBranch::try_new("track/cleanup-test").unwrap();
        let metadata = TrackMetadata::with_branch(
            track_id,
            Some(branch),
            "Cleanup test",
            None,
            BranchStrategySnapshot::new(
                NonEmptyString::try_new("develop").unwrap(),
                NonEmptyString::try_new("develop").unwrap(),
                MergeMethod::Merge,
            ),
        )
        .unwrap();
        derive_base_merge_direction(&metadata).unwrap()
    }

    struct FixedCleanupContext;

    impl BaseMergeContextPort for FixedCleanupContext {
        fn load_direction(
            &self,
            _workspace_root: &Path,
        ) -> Result<BaseMergeDirection, BaseMergeContextError> {
            Ok(cleanup_direction())
        }
    }

    struct ConflictedCleanupGit;

    impl BaseMergeGitPort for ConflictedCleanupGit {
        fn merge_base(
            &self,
            _workspace_root: &Path,
            _direction: &BaseMergeDirection,
        ) -> Result<BaseMergeAttemptOutcome, BaseMergeGitError> {
            Ok(BaseMergeAttemptOutcome::Conflicted)
        }
    }

    struct CleanCleanupGit;

    impl BaseMergeGitPort for CleanCleanupGit {
        fn merge_base(
            &self,
            _workspace_root: &Path,
            _direction: &BaseMergeDirection,
        ) -> Result<BaseMergeAttemptOutcome, BaseMergeGitError> {
            Ok(BaseMergeAttemptOutcome::Clean {
                base_commit: CommitHash::try_new("0123456789abcdef").unwrap(),
            })
        }
    }

    struct ExactCommitCleanupGit {
        base_commit: CommitHash,
    }

    impl BaseMergeGitPort for ExactCommitCleanupGit {
        fn merge_base(
            &self,
            _workspace_root: &Path,
            _direction: &BaseMergeDirection,
        ) -> Result<BaseMergeAttemptOutcome, BaseMergeGitError> {
            Ok(BaseMergeAttemptOutcome::Clean { base_commit: self.base_commit.clone() })
        }
    }

    struct FixtureBaselineCapture;

    impl domain::tddd::catalogue_v2::RustdocBaselineCapturePort for FixtureBaselineCapture {
        fn capture(
            &self,
            items_dir: &Path,
            track_id: &TrackId,
            rustdoc_workspace: &Path,
            binding: &domain::tddd::catalogue_v2::TdddLayerBinding,
            _features: &[domain::tddd::CargoFeatureName],
        ) -> Result<(), domain::tddd::catalogue_v2::BaselineCaptureIoError> {
            let source = rustdoc_workspace.join("libs/domain/src/lib.rs");
            let marker = std::fs::read_to_string(&source).map_err(|error| {
                domain::tddd::catalogue_v2::BaselineCaptureIoError(error.to_string())
            })?;
            let baseline = serde_json::json!({
                "root": 0,
                "crate_version": marker,
                "includes_private": false,
                "index": {},
                "paths": {},
                "external_crates": {},
                "format_version": rustdoc_types::FORMAT_VERSION,
                "target": {"triple": "", "target_features": []}
            })
            .to_string();
            let target = items_dir.join(track_id.as_ref()).join(&binding.baseline_file);
            std::fs::write(target, baseline).map_err(|error| {
                domain::tddd::catalogue_v2::BaselineCaptureIoError(error.to_string())
            })
        }
    }

    struct ConcurrentWriteBaselineCapture {
        live_track: std::path::PathBuf,
    }

    impl domain::tddd::catalogue_v2::RustdocBaselineCapturePort for ConcurrentWriteBaselineCapture {
        fn capture(
            &self,
            items_dir: &Path,
            track_id: &TrackId,
            rustdoc_workspace: &Path,
            binding: &domain::tddd::catalogue_v2::TdddLayerBinding,
            features: &[domain::tddd::CargoFeatureName],
        ) -> Result<(), domain::tddd::catalogue_v2::BaselineCaptureIoError> {
            std::fs::write(
                self.live_track.join("concurrent-review-state.json"),
                "written while baseline capture was running",
            )
            .map_err(|error| {
                domain::tddd::catalogue_v2::BaselineCaptureIoError(error.to_string())
            })?;
            FixtureBaselineCapture.capture(
                items_dir,
                track_id,
                rustdoc_workspace,
                binding,
                features,
            )
        }
    }

    #[test]
    fn test_fs_base_merge_context_loads_authoritative_snapshot_direction() {
        let fixture = setup_repository("adapter-test", "develop");

        let direction = FsBaseMergeContextAdapter::new().load_direction(fixture.path()).unwrap();

        assert_eq!(direction.track_id().as_ref(), "adapter-test");
        assert_eq!(direction.active_track().as_ref(), "track/adapter-test");
        assert_eq!(direction.source().as_str(), "develop");
    }

    #[test]
    fn test_fs_base_merge_context_reloads_the_snapshot_base_from_metadata() {
        let fixture = setup_repository("adapter-test", "develop");

        write_metadata(fixture.path(), "adapter-test", "release");
        let direction = FsBaseMergeContextAdapter::new().load_direction(fixture.path()).unwrap();

        assert_eq!(direction.source().as_str(), "release");
    }

    #[test]
    fn test_fs_base_merge_context_rejects_non_track_and_nested_workspace() {
        let fixture = setup_repository("adapter-test", "develop");
        let adapter = FsBaseMergeContextAdapter::new();

        git(fixture.path(), &["switch", "--quiet", "develop"]);
        assert!(matches!(
            adapter.load_direction(fixture.path()),
            Err(BaseMergeContextError::Unavailable(_))
        ));

        git(fixture.path(), &["switch", "--quiet", "track/adapter-test"]);
        assert!(matches!(
            adapter.load_direction(&fixture.path().join("track")),
            Err(BaseMergeContextError::Unavailable(_))
        ));
    }

    #[test]
    fn test_fs_base_merge_context_rejects_missing_and_malformed_metadata() {
        let fixture = setup_repository("adapter-test", "develop");
        let metadata_path = fixture.path().join("track/items/adapter-test/metadata.json");
        let adapter = FsBaseMergeContextAdapter::new();

        std::fs::remove_file(&metadata_path).unwrap();
        assert!(matches!(
            adapter.load_direction(fixture.path()),
            Err(BaseMergeContextError::Unavailable(_))
        ));

        std::fs::write(&metadata_path, "{malformed").unwrap();
        assert!(matches!(
            adapter.load_direction(fixture.path()),
            Err(BaseMergeContextError::Unavailable(_))
        ));
    }

    #[test]
    fn test_fs_base_merge_git_resolves_snapshot_branch_when_tag_has_same_name() {
        let fixture = setup_repository("adapter-test", "develop");
        let root = fixture.path();
        let direction = FsBaseMergeContextAdapter::new().load_direction(root).unwrap();
        let branch_commit = current_commit(root, "refs/heads/develop^{commit}");

        git(root, &["tag", "develop", "refs/heads/track/adapter-test"]);
        let tag_commit = current_commit(root, "refs/tags/develop^{commit}");
        assert_ne!(branch_commit, tag_commit, "the fixture must contain distinct refs");

        let outcome = FsBaseMergeGitAdapter::new().merge_base(root, &direction).unwrap();

        assert_eq!(
            outcome,
            BaseMergeAttemptOutcome::Clean {
                base_commit: CommitHash::try_new(branch_commit).unwrap()
            }
        );
    }

    #[test]
    fn test_fs_base_merge_git_merges_snapshot_source_with_exact_base_commit() {
        let fixture = setup_repository("adapter-test", "develop");
        let root = fixture.path();
        let direction = FsBaseMergeContextAdapter::new().load_direction(root).unwrap();
        let expected_commit = current_commit(root, "develop^{commit}");

        let outcome = FsBaseMergeGitAdapter::new().merge_base(root, &direction).unwrap();

        assert_eq!(
            outcome,
            BaseMergeAttemptOutcome::Clean {
                base_commit: CommitHash::try_new(expected_commit).unwrap()
            }
        );
        assert!(root.join("base.txt").is_file(), "the base branch content must be merged");
    }

    #[test]
    fn test_run_guarded_merge_uses_resolved_commit_after_source_branch_advances() {
        let fixture = setup_repository("adapter-test", "develop");
        let root = fixture.path();
        let resolved = CommitHash::try_new(current_commit(root, "develop^{commit}")).unwrap();

        git(root, &["switch", "--quiet", "develop"]);
        std::fs::write(root.join("advanced-after-resolution.txt"), "later\n").unwrap();
        git(root, &["add", "advanced-after-resolution.txt"]);
        git(root, &["commit", "--quiet", "-m", "advance base after resolution"]);
        git(root, &["switch", "--quiet", "track/adapter-test"]);

        let output = run_guarded_merge(root, &resolved).unwrap();

        assert!(output.status.success());
        assert!(root.join("base.txt").is_file());
        assert!(
            !root.join("advanced-after-resolution.txt").exists(),
            "the merge must use the resolved commit rather than the moved branch"
        );
    }

    #[test]
    fn test_fs_base_merge_git_preserves_typed_conflicts_without_cleanup() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        git(root, &["init", "--quiet", "--initial-branch=develop"]);
        git(root, &["config", "user.email", "test@example.com"]);
        git(root, &["config", "user.name", "Base Merge Test"]);
        std::fs::write(root.join("conflict.txt"), "initial\n").unwrap();
        git(root, &["add", "conflict.txt"]);
        git(root, &["commit", "--quiet", "-m", "initial"]);
        git(root, &["switch", "--quiet", "-c", "track/conflict-test"]);
        std::fs::write(root.join("conflict.txt"), "track\n").unwrap();
        git(root, &["add", "conflict.txt"]);
        git(root, &["commit", "--quiet", "-m", "track conflict"]);
        git(root, &["switch", "--quiet", "develop"]);
        std::fs::write(root.join("conflict.txt"), "base\n").unwrap();
        git(root, &["add", "conflict.txt"]);
        git(root, &["commit", "--quiet", "-m", "base conflict"]);
        git(root, &["switch", "--quiet", "track/conflict-test"]);
        write_metadata(root, "conflict-test", "develop");
        let direction = FsBaseMergeContextAdapter::new().load_direction(root).unwrap();

        let outcome = FsBaseMergeGitAdapter::new().merge_base(root, &direction).unwrap();

        assert_eq!(outcome, BaseMergeAttemptOutcome::Conflicted);
        let status = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&status.stdout).contains("UU conflict.txt"));
    }

    #[test]
    fn test_fs_base_merge_git_rejects_pre_existing_merge_in_progress() {
        let fixture = setup_repository("adapter-test", "develop");
        let root = fixture.path();
        let direction = FsBaseMergeContextAdapter::new().load_direction(root).unwrap();

        let head = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .unwrap();
        std::fs::write(
            root.join(".git/MERGE_HEAD"),
            String::from_utf8_lossy(&head.stdout).as_bytes(),
        )
        .unwrap();
        let result = FsBaseMergeGitAdapter::new().merge_base(root, &direction);

        assert!(
            matches!(result, Err(BaseMergeGitError::Execution(_))),
            "a pre-existing MERGE_HEAD must refuse the merge instead of adjudicating it"
        );
    }

    #[test]
    fn test_fs_base_merge_git_rejects_malformed_pre_existing_merge_head() {
        let fixture = setup_repository("adapter-test", "develop");
        let root = fixture.path();
        let direction = FsBaseMergeContextAdapter::new().load_direction(root).unwrap();

        std::fs::write(root.join(".git/MERGE_HEAD"), "not-a-commit\n").unwrap();
        let result = FsBaseMergeGitAdapter::new().merge_base(root, &direction);

        assert!(
            matches!(result, Err(BaseMergeGitError::Execution(_))),
            "a malformed MERGE_HEAD must fail closed instead of being treated as absent"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_open_base_merge_lock_file_rejects_symlink() {
        let fixture = tempfile::tempdir().unwrap();
        let target = fixture.path().join("target.lock");
        let link = fixture.path().join("metadata.json.lock");
        std::fs::write(&target, b"target").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(open_base_merge_lock_file(&link).is_err());
    }

    #[test]
    fn test_fs_base_merge_git_rejects_pre_existing_unmerged_paths() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        git(root, &["init", "--quiet", "--initial-branch=develop"]);
        git(root, &["config", "user.email", "test@example.com"]);
        git(root, &["config", "user.name", "Base Merge Test"]);
        std::fs::write(root.join("conflict.txt"), "initial\n").unwrap();
        git(root, &["add", "conflict.txt"]);
        git(root, &["commit", "--quiet", "-m", "initial"]);
        git(root, &["switch", "--quiet", "-c", "track/conflict-test"]);
        std::fs::write(root.join("conflict.txt"), "track\n").unwrap();
        git(root, &["add", "conflict.txt"]);
        git(root, &["commit", "--quiet", "-m", "track conflict"]);
        git(root, &["switch", "--quiet", "develop"]);
        std::fs::write(root.join("conflict.txt"), "base\n").unwrap();
        git(root, &["add", "conflict.txt"]);
        git(root, &["commit", "--quiet", "-m", "base conflict"]);
        git(root, &["switch", "--quiet", "track/conflict-test"]);
        write_metadata(root, "conflict-test", "develop");
        let direction = FsBaseMergeContextAdapter::new().load_direction(root).unwrap();
        // Establish unmerged entries from an unrelated merge, then drop its
        // MERGE_HEAD so only the unmerged index remains.
        let merge = std::process::Command::new("git")
            .args(["merge", "--no-ff", "--no-edit", "develop"])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(!merge.status.success(), "the fixture merge must conflict");
        std::fs::remove_file(root.join(".git/MERGE_HEAD")).unwrap();

        let base_commit = resolve_base_commit(root, direction.source()).unwrap();
        let adjudicated = adjudicate_merge_after_runner_error(root, base_commit);
        assert!(
            matches!(adjudicated, Err(BaseMergeGitError::Execution(_))),
            "unmerged paths without this merge's MERGE_HEAD must fail closed"
        );

        let result = FsBaseMergeGitAdapter::new().merge_base(root, &direction);

        assert!(
            matches!(result, Err(BaseMergeGitError::Execution(_))),
            "pre-existing unmerged paths must refuse the merge instead of reporting Conflicted"
        );
    }

    #[test]
    fn test_adjudicate_merge_after_runner_error_requires_matching_merge_head_and_unmerged_paths() {
        let fixture = setup_repository("adapter-test", "develop");
        let root = fixture.path();

        git(root, &["switch", "--quiet", "develop"]);
        std::fs::write(root.join("shared.txt"), "base conflict\n").unwrap();
        git(root, &["add", "shared.txt"]);
        git(root, &["commit", "--quiet", "-m", "base conflict"]);
        git(root, &["switch", "--quiet", "track/adapter-test"]);
        std::fs::write(root.join("shared.txt"), "track conflict\n").unwrap();
        git(root, &["add", "shared.txt"]);
        git(root, &["commit", "--quiet", "-m", "track conflict"]);

        let direction = FsBaseMergeContextAdapter::new().load_direction(root).unwrap();
        let base_commit = resolve_base_commit(root, direction.source()).unwrap();
        let merge = std::process::Command::new("git")
            .args(["merge", "--no-ff", "--no-edit", base_commit.as_ref()])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(!merge.status.success(), "the fixture merge must conflict");
        let merge_head = root.join(".git/MERGE_HEAD");

        assert_eq!(
            adjudicate_merge_after_runner_error(root, base_commit.clone()).unwrap(),
            BaseMergeAttemptOutcome::Conflicted,
            "a matching MERGE_HEAD plus unmerged paths identifies the runner-created conflict"
        );

        git(root, &["merge", "--abort"]);
        std::fs::write(&merge_head, format!("{}\n", base_commit.as_ref())).unwrap();
        assert!(
            matches!(
                adjudicate_merge_after_runner_error(root, base_commit.clone()),
                Err(BaseMergeGitError::Execution(_))
            ),
            "a matching MERGE_HEAD without unmerged paths must fail closed"
        );

        let mismatched_commit = current_commit(root, "HEAD^{commit}");
        assert_ne!(mismatched_commit, base_commit.as_ref());
        std::fs::write(&merge_head, format!("{mismatched_commit}\n")).unwrap();
        assert!(
            matches!(
                adjudicate_merge_after_runner_error(root, base_commit),
                Err(BaseMergeGitError::Execution(_))
            ),
            "a mismatched MERGE_HEAD must fail closed"
        );
    }

    #[test]
    fn test_fs_base_merge_git_rejects_branch_change_before_merge() {
        let fixture = setup_repository("adapter-test", "develop");
        let root = fixture.path();
        let direction = FsBaseMergeContextAdapter::new().load_direction(root).unwrap();

        git(root, &["switch", "--quiet", "develop"]);
        let result = FsBaseMergeGitAdapter::new().merge_base(root, &direction);

        assert!(matches!(result, Err(BaseMergeGitError::Execution(_))));
        assert!(!root.join("track.txt").is_file(), "the reverse direction must not be merged");
    }

    #[test]
    fn test_fs_base_merge_git_rejects_snapshot_branch_provenance_mismatch() {
        let fixture = setup_repository("adapter-test", "develop");
        let root = fixture.path();
        let direction = FsBaseMergeContextAdapter::new().load_direction(root).unwrap();

        std::fs::write(
            root.join("track/items/adapter-test/metadata.json"),
            r#"{
  "schema_version": 6,
  "id": "adapter-test",
  "branch": "track/a-different-track",
  "title": "Adapter fixture",
  "created_at": "2026-08-02T00:00:00Z",
  "updated_at": "2026-08-02T00:00:00Z",
  "branch_strategy_snapshot": {
    "base_branch": "develop",
    "merge_target": "develop",
    "merge_method": "merge"
  }
}"#,
        )
        .unwrap();

        let result = FsBaseMergeGitAdapter::new().merge_base(root, &direction);

        assert!(matches!(result, Err(BaseMergeGitError::Execution(_))));
        assert!(!root.join("base.txt").is_file(), "the merge must not start");
    }

    #[test]
    fn test_fs_base_merge_git_rejects_supplied_source_different_from_snapshot_base() {
        let fixture = setup_repository("adapter-test", "develop");
        let root = fixture.path();
        let supplied_direction = FsBaseMergeContextAdapter::new().load_direction(root).unwrap();

        write_metadata(root, "adapter-test", "release");
        let result = FsBaseMergeGitAdapter::new().merge_base(root, &supplied_direction);

        assert!(matches!(result, Err(BaseMergeGitError::Execution(_))));
        assert!(!root.join("base.txt").is_file(), "the merge must not start");
    }

    #[test]
    fn test_fs_base_merge_git_maps_missing_snapshot_source_to_typed_error() {
        let fixture = setup_repository("adapter-test", "missing-base");
        let root = fixture.path();
        let direction = FsBaseMergeContextAdapter::new().load_direction(root).unwrap();

        let result = FsBaseMergeGitAdapter::new().merge_base(root, &direction);

        assert!(matches!(result, Err(BaseMergeGitError::Execution(_))));
    }

    #[test]
    fn test_fs_base_merge_cleanup_baseline_replacement_isolated_at_exact_commit_and_atomically_published()
     {
        let fixture = setup_repository("cleanup-test", "develop");
        let root = fixture.path();
        let expected_commit = current_commit(root, "develop^{commit}");
        let worktree = create_unique_directory(root, ".test-base-merge-worktree-").unwrap();

        add_commit_pinned_worktree(
            root,
            &worktree,
            &CommitHash::try_new(expected_commit.clone()).unwrap(),
        )
        .unwrap();
        assert_eq!(current_commit(&worktree, "HEAD^{commit}"), expected_commit);

        let track_dir = root.join("track/items/cleanup-test");
        let recovery_root = root.join("track/.sotp-baseline-recovery");
        std::fs::create_dir_all(&recovery_root).unwrap();
        let replacement = recovery_root.join("cleanup-test");
        std::fs::create_dir(&replacement).unwrap();
        let baseline = track_dir.join("domain-types-baseline.json");
        let stale_baseline = track_dir.join("obsolete-types-baseline.json");
        let type_signals = track_dir.join("domain-type-signals.json");
        std::fs::write(&baseline, "prior-valid-baseline").unwrap();
        std::fs::write(&stale_baseline, "obsolete-baseline").unwrap();
        std::fs::write(&type_signals, "preserved-cache").unwrap();
        replace_tree(&track_dir, &replacement, true, &recovery_root).unwrap();
        write_replacement_phase_marker(&replacement).unwrap();
        std::fs::write(replacement.join("domain-types-baseline.json"), "replacement-baseline")
            .unwrap();
        let _writer_lock =
            acquire_track_writer_lock(&track_dir, &root.join("track/items")).unwrap();

        let mut exchanged = false;
        let generated_baseline_files = BTreeSet::from(["domain-types-baseline.json".to_owned()]);
        publish_baseline_replacements(
            &track_dir,
            &replacement,
            &generated_baseline_files,
            &mut exchanged,
        )
        .unwrap();
        assert!(exchanged);

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            let active_lock = std::fs::metadata(track_dir.join(TRACK_WRITER_LOCK_FILE)).unwrap();
            let recovery_lock =
                std::fs::metadata(replacement.join(TRACK_WRITER_LOCK_FILE)).unwrap();
            assert_eq!(
                active_lock.ino(),
                recovery_lock.ino(),
                "the exchanged active and recovery paths must retain one writer lock inode"
            );
        }
        assert_eq!(std::fs::read_to_string(&baseline).unwrap(), "replacement-baseline");
        assert_eq!(
            std::fs::read_to_string(&stale_baseline).unwrap(),
            "obsolete-baseline",
            "an unconfigured root baseline must be preserved"
        );
        assert_eq!(std::fs::read_to_string(&type_signals).unwrap(), "preserved-cache");
        assert!(replacement.is_dir(), "recovery slot must retain the prior track");
        let stamp_request = usecase::base_merge::BaseMergeCleanupRequest {
            workspace_root: root.to_path_buf(),
            track_id: TrackId::try_new("cleanup-test").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new(expected_commit).unwrap(),
        };
        write_sync_base_record_atomically(&stamp_request).unwrap();
        let stamp =
            decode(&std::fs::read_to_string(track_dir.join(".sync-base.json")).unwrap()).unwrap();
        assert_eq!(stamp.base_commit, stamp_request.base_commit);
        assert!(!replacement.exists(), "successful SyncBase must remove the recovery slot");
        remove_commit_pinned_worktree(root, &worktree).unwrap();
        let _ = std::fs::remove_dir_all(&worktree);
    }

    #[test]
    fn test_fs_base_merge_cleanup_rejects_concurrent_non_baseline_write_and_preserves_it() {
        let fixture = setup_cleanup_repository();
        let root = fixture.path();
        let track_dir = root.join("track/items/cleanup-test");
        let prior_baseline = track_dir.join("domain-types-baseline.json");
        std::fs::write(&prior_baseline, "prior-valid-baseline").unwrap();
        std::fs::write(track_dir.join("preserved-input.txt"), "prior-complete-track-input")
            .unwrap();
        let request = usecase::base_merge::BaseMergeCleanupRequest {
            workspace_root: root.to_path_buf(),
            track_id: TrackId::try_new("cleanup-test").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new(current_commit(root, "develop^{commit}")).unwrap(),
        };
        let adapter = FsBaseMergeCleanupAdapter::with_baseline_capture(Arc::new(
            ConcurrentWriteBaselineCapture { live_track: track_dir.clone() },
        ));

        let result = adapter.replace_baselines(&request);

        assert!(
            matches!(result, Err(BaselineReplacementError::Publish(_))),
            "concurrent non-baseline writes must abort publication before exchange"
        );
        assert_eq!(std::fs::read_to_string(&prior_baseline).unwrap(), "prior-valid-baseline");
        assert_eq!(
            std::fs::read_to_string(track_dir.join("preserved-input.txt")).unwrap(),
            "prior-complete-track-input"
        );
        assert_eq!(
            std::fs::read_to_string(track_dir.join("concurrent-review-state.json")).unwrap(),
            "written while baseline capture was running"
        );
    }

    #[test]
    fn test_copy_tree_preserves_nested_metadata_lock_file() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let source_root = source.path().join("track");
        let target_root = target.path().join("staged");
        let root_baseline = source_root.join("root-types-baseline.json");
        let nested_lock = source_root.join("artifacts/metadata.json.lock");
        let nested_baseline = source_root.join("artifacts/custom-baseline.json");
        let nested_types_baseline = source_root.join("artifacts/nested-types-baseline.json");
        std::fs::create_dir_all(nested_lock.parent().unwrap()).unwrap();
        std::fs::write(&root_baseline, "root baseline").unwrap();
        std::fs::write(&nested_lock, "nested lock content").unwrap();
        std::fs::write(&nested_baseline, "nested baseline content").unwrap();
        std::fs::write(&nested_types_baseline, "nested types baseline content").unwrap();

        let generated_baseline_files = BTreeSet::from(["root-types-baseline.json".to_owned()]);
        cleanup_tree::copy_tree_with_baselines(
            &source_root,
            &target_root,
            false,
            target.path(),
            &generated_baseline_files,
        )
        .unwrap();

        assert!(!target_root.join("root-types-baseline.json").exists());
        assert_eq!(
            std::fs::read_to_string(target_root.join("artifacts/metadata.json.lock")).unwrap(),
            "nested lock content"
        );
        assert_eq!(
            std::fs::read_to_string(target_root.join("artifacts/custom-baseline.json")).unwrap(),
            "nested baseline content"
        );
        assert_eq!(
            std::fs::read_to_string(target_root.join("artifacts/nested-types-baseline.json"))
                .unwrap(),
            "nested types baseline content"
        );
    }

    #[test]
    fn test_fs_base_merge_cleanup_adapter_regenerates_views_replaces_pinned_baselines_and_preserves_type_signals()
     {
        let fixture = setup_cleanup_repository();
        let root = fixture.path();
        let track_dir = root.join("track/items/cleanup-test");
        let type_signals = track_dir.join("domain-type-signals.json");
        let prior_baseline = track_dir.join("domain-types-baseline.json");
        let features = track_dir.join("tddd-features.json");
        std::fs::write(&type_signals, "preserved-cache").unwrap();
        std::fs::write(&prior_baseline, "prior-baseline").unwrap();
        let requested_base_commit = current_commit(root, "develop^{commit}");
        git(root, &["switch", "--quiet", "develop"]);
        std::fs::write(
            root.join("libs/domain/src/lib.rs"),
            "pub struct CleanupFixture;\npub struct AdvancedDevelopOnlyMarker;\n",
        )
        .unwrap();
        git(root, &["add", "libs/domain/src/lib.rs"]);
        git(root, &["commit", "--quiet", "-m", "advance develop after requested base"]);
        let advanced_develop_commit = current_commit(root, "develop^{commit}");
        assert_ne!(requested_base_commit, advanced_develop_commit);
        git(root, &["switch", "--quiet", "track/cleanup-test"]);
        let request = usecase::base_merge::BaseMergeCleanupRequest {
            workspace_root: root.to_path_buf(),
            track_id: TrackId::try_new("cleanup-test").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new(requested_base_commit.clone()).unwrap(),
        };
        let adapter =
            FsBaseMergeCleanupAdapter::with_baseline_capture(Arc::new(FixtureBaselineCapture));

        adapter.regenerate_views(&request).unwrap();
        assert!(track_dir.join("plan.md").is_file(), "view regeneration must publish plan.md");
        adapter.replace_baselines(&request).unwrap();

        let published_baseline = std::fs::read_to_string(&prior_baseline).unwrap();
        assert_ne!(published_baseline, "prior-baseline");
        crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec::from_json(&published_baseline)
            .unwrap();
        assert!(
            published_baseline.contains("CleanupFixture"),
            "the published baseline must come from the requested commit"
        );
        assert!(
            !published_baseline.contains("AdvancedDevelopOnlyMarker"),
            "the advanced develop branch must not be resolved after the merge commit is fixed"
        );
        assert_eq!(std::fs::read_to_string(&type_signals).unwrap(), "preserved-cache");
        assert!(features.is_file(), "atomic publication must retain the complete active track");
        let recovery_slot = root.join("track/.sotp-baseline-recovery/cleanup-test");
        assert!(
            recovery_slot.is_dir(),
            "atomic publication must retain the complete prior track outside active items"
        );
        adapter.write_sync_base_record(&request).unwrap();
        let record =
            decode(&std::fs::read_to_string(track_dir.join(".sync-base.json")).unwrap()).unwrap();
        assert_eq!(record.base_commit, request.base_commit);
        assert_eq!(record.base_commit.as_ref(), requested_base_commit);
        assert!(
            !recovery_slot.exists(),
            "successful SyncBase publication must prune the recovery copy"
        );
    }

    #[test]
    fn test_fs_base_merge_cleanup_retains_writer_lock_until_sync_base_record() {
        let fixture = setup_cleanup_repository();
        let root = fixture.path();
        let track_dir = root.join("track/items/cleanup-test");
        std::fs::write(track_dir.join("domain-types-baseline.json"), "prior-baseline").unwrap();
        let request = usecase::base_merge::BaseMergeCleanupRequest {
            workspace_root: root.to_path_buf(),
            track_id: TrackId::try_new("cleanup-test").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new(current_commit(root, "develop^{commit}")).unwrap(),
        };
        let first =
            FsBaseMergeCleanupAdapter::with_baseline_capture(Arc::new(FixtureBaselineCapture));
        first.replace_baselines(&request).unwrap();

        let second =
            FsBaseMergeCleanupAdapter::with_baseline_capture(Arc::new(FixtureBaselineCapture));
        let second_result = second.replace_baselines(&request);

        assert!(
            matches!(second_result, Err(BaselineReplacementError::Publish(_))),
            "second transaction result: {second_result:?}"
        );
        first.write_sync_base_record(&request).unwrap();
        assert!(!root.join("track/.sotp-baseline-recovery/cleanup-test").exists());
    }

    #[test]
    fn test_fs_base_merge_cleanup_sync_stamp_is_schema_versioned_and_idempotent() {
        let fixture = tempfile::tempdir().unwrap();
        let track_dir = fixture.path().join("track/items/cleanup-test");
        std::fs::create_dir_all(&track_dir).unwrap();
        let request = usecase::base_merge::BaseMergeCleanupRequest {
            workspace_root: fixture.path().to_path_buf(),
            track_id: TrackId::try_new("cleanup-test").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new("0123456789abcdef").unwrap(),
        };
        let stale_recovery = fixture.path().join("track/.sotp-baseline-recovery/cleanup-test");
        std::fs::create_dir_all(&stale_recovery).unwrap();
        std::fs::write(stale_recovery.join("marker"), "stale").unwrap();

        write_sync_base_record_atomically(&request).unwrap();
        write_sync_base_record_atomically(&request).unwrap();

        let encoded = std::fs::read_to_string(track_dir.join(".sync-base.json")).unwrap();
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded.schema_version, SyncBaseRecordSchemaVersion::V1);
        assert_eq!(decoded.track_id, request.track_id);
        assert_eq!(decoded.base_branch, request.base_branch);
        assert_eq!(decoded.base_commit, request.base_commit);
        assert!(
            !stale_recovery.exists(),
            "idempotent writes must still prune stale recovery copies"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_base_merge_cleanup_sync_stamp_rejects_symlinked_recovery_root() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        let track_dir = root.join("track/items/cleanup-test");
        std::fs::create_dir_all(&track_dir).unwrap();
        let outside = root.join("outside-recovery");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("marker"), "must-survive").unwrap();
        let recovery_root = root.join("track/.sotp-baseline-recovery");
        std::os::unix::fs::symlink(&outside, &recovery_root).unwrap();
        let request = usecase::base_merge::BaseMergeCleanupRequest {
            workspace_root: root.to_path_buf(),
            track_id: TrackId::try_new("cleanup-test").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new("0123456789abcdef").unwrap(),
        };

        let result = write_sync_base_record_atomically(&request);

        assert!(matches!(result, Err(SyncBaseRecordError::Validation(_))));
        assert_eq!(std::fs::read_to_string(outside.join("marker")).unwrap(), "must-survive");
        assert!(
            !track_dir.join(".sync-base.json").exists(),
            "a rejected recovery boundary must not publish the SyncBase record"
        );
        assert!(recovery_root.is_symlink(), "the recovery-root symlink must remain untouched");
    }

    #[test]
    fn test_fs_base_merge_cleanup_adapter_propagates_view_regeneration_failure() {
        let fixture = tempfile::tempdir().unwrap();
        let track_dir = fixture.path().join("track/items/cleanup-test");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), "{malformed").unwrap();
        let request = usecase::base_merge::BaseMergeCleanupRequest {
            workspace_root: fixture.path().to_path_buf(),
            track_id: TrackId::try_new("cleanup-test").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new("0123456789abcdef").unwrap(),
        };

        let result = FsBaseMergeCleanupAdapter::new().regenerate_views(&request);

        assert!(matches!(result, Err(ViewsRegenerationError::Regeneration(_))));
    }

    #[test]
    fn test_base_merge_interactor_clean_merge_view_failure_preserves_baseline_and_sync_stamp() {
        let fixture = tempfile::tempdir().unwrap();
        let track_dir = fixture.path().join("track/items/cleanup-test");
        std::fs::create_dir_all(&track_dir).unwrap();
        let prior_baseline = track_dir.join("domain-types-baseline.json");
        std::fs::write(&prior_baseline, "prior-valid-baseline").unwrap();
        std::fs::write(track_dir.join("track-input.json"), "prior-complete-track-input").unwrap();
        std::fs::write(track_dir.join("metadata.json"), "{malformed").unwrap();

        let interactor = usecase::base_merge::BaseMergeInteractor::new(
            Arc::new(FixedCleanupContext),
            Arc::new(CleanCleanupGit),
            Arc::new(FsBaseMergeCleanupAdapter::new()),
        );

        let result = usecase::base_merge::BaseMergeService::execute(
            &interactor,
            usecase::base_merge::BaseMergeCommand { workspace_root: fixture.path().to_path_buf() },
        );

        assert!(matches!(
            result,
            Err(usecase::base_merge::BaseMergeError::PostMergeCleanup(
                usecase::base_merge::PostMergeCleanupError::Views(
                    ViewsRegenerationError::Regeneration(_)
                )
            ))
        ));
        assert_eq!(std::fs::read_to_string(&prior_baseline).unwrap(), "prior-valid-baseline");
        assert_eq!(
            std::fs::read_to_string(track_dir.join("track-input.json")).unwrap(),
            "prior-complete-track-input"
        );
        assert!(
            !track_dir.join(".sync-base.json").exists(),
            "the SyncBase stage must not run after Views fails"
        );
    }

    #[test]
    fn test_base_merge_interactor_clean_merge_baseline_failure_preserves_baseline_and_sync_stamp() {
        let fixture = setup_cleanup_repository();
        let root = fixture.path();
        let track_dir = root.join("track/items/cleanup-test");
        let prior_baseline = track_dir.join("domain-types-baseline.json");
        std::fs::write(&prior_baseline, "prior-valid-baseline").unwrap();
        std::fs::write(track_dir.join("track-input.json"), "prior-complete-track-input").unwrap();

        let interactor = usecase::base_merge::BaseMergeInteractor::new(
            Arc::new(FixedCleanupContext),
            Arc::new(CleanCleanupGit),
            Arc::new(FsBaseMergeCleanupAdapter::new()),
        );

        let result = usecase::base_merge::BaseMergeService::execute(
            &interactor,
            usecase::base_merge::BaseMergeCommand { workspace_root: root.to_path_buf() },
        );

        assert!(matches!(
            result,
            Err(usecase::base_merge::BaseMergeError::PostMergeCleanup(
                usecase::base_merge::PostMergeCleanupError::Baseline(
                    BaselineReplacementError::Isolation(_)
                )
            ))
        ));
        assert!(track_dir.join("plan.md").is_file(), "Views must complete before Baseline");
        assert_eq!(std::fs::read_to_string(&prior_baseline).unwrap(), "prior-valid-baseline");
        assert_eq!(
            std::fs::read_to_string(track_dir.join("track-input.json")).unwrap(),
            "prior-complete-track-input"
        );
        assert!(
            !track_dir.join(".sync-base.json").exists(),
            "the SyncBase stage must not run after Baseline fails"
        );
    }

    #[test]
    fn test_base_merge_interactor_sync_stamp_failure_preserves_complete_published_track() {
        let fixture = setup_cleanup_repository();
        let root = fixture.path();
        let track_dir = root.join("track/items/cleanup-test");
        let baseline = track_dir.join("domain-types-baseline.json");
        let type_signals = track_dir.join("domain-type-signals.json");
        let base_commit = CommitHash::try_new(current_commit(root, "develop^{commit}")).unwrap();
        std::fs::write(&baseline, "prior-baseline").unwrap();
        std::fs::write(&type_signals, "preserved-cache").unwrap();
        let sync_stamp = track_dir.join(".sync-base.json");
        std::fs::create_dir(&sync_stamp).unwrap();

        let interactor = usecase::base_merge::BaseMergeInteractor::new(
            Arc::new(FixedCleanupContext),
            Arc::new(ExactCommitCleanupGit { base_commit }),
            Arc::new(FsBaseMergeCleanupAdapter::with_baseline_capture(Arc::new(
                FixtureBaselineCapture,
            ))),
        );

        let result = usecase::base_merge::BaseMergeService::execute(
            &interactor,
            usecase::base_merge::BaseMergeCommand { workspace_root: root.to_path_buf() },
        );

        assert!(matches!(
            &result,
            Err(usecase::base_merge::BaseMergeError::PostMergeCleanup(
                usecase::base_merge::PostMergeCleanupError::SyncBaseStamp(
                    SyncBaseRecordError::Write(_)
                )
            ))
        ));
        assert!(
            !matches!(result, Ok(usecase::base_merge::BaseMergeOutcome::Completed)),
            "a failed SyncBase stage must never report completed cleanup"
        );
        assert!(track_dir.join("plan.md").is_file(), "Views must complete before SyncBase");
        let published_baseline = std::fs::read_to_string(&baseline).unwrap();
        assert_ne!(published_baseline, "prior-baseline");
        crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec::from_json(&published_baseline)
            .unwrap();
        assert!(
            track_dir.join("tddd-features.json").is_file(),
            "the published baseline replacement must retain the complete track"
        );
        assert_eq!(std::fs::read_to_string(type_signals).unwrap(), "preserved-cache");
        assert!(sync_stamp.is_dir(), "the failed sync target must remain unchanged");
        assert!(
            root.join("track/.sotp-baseline-recovery/cleanup-test").is_dir(),
            "a failed SyncBase validation must retain the canonical recovery copy"
        );
        assert!(
            !root.join("track/.sotp-baseline-recovery-cleanup-test").exists(),
            "a failed SyncBase validation must not create a pending copy"
        );
    }

    #[test]
    fn test_base_merge_retry_reconciles_retained_recovery_after_telemetry_drift() {
        let fixture = setup_cleanup_repository();
        let root = fixture.path();
        let track_dir = root.join("track/items/cleanup-test");
        let sync_stamp = track_dir.join(".sync-base.json");
        let telemetry = track_dir.join("failure-telemetry.json");
        let base_commit = CommitHash::try_new(current_commit(root, "develop^{commit}")).unwrap();
        std::fs::write(&telemetry, "initial failure\n").unwrap();
        std::fs::create_dir(&sync_stamp).unwrap();
        let cleanup: Arc<dyn BaseMergeCleanupPort> = Arc::new(
            FsBaseMergeCleanupAdapter::with_baseline_capture(Arc::new(FixtureBaselineCapture)),
        );
        let run = || {
            let interactor = usecase::base_merge::BaseMergeInteractor::new(
                Arc::new(FixedCleanupContext),
                Arc::new(ExactCommitCleanupGit { base_commit: base_commit.clone() }),
                Arc::clone(&cleanup),
            );
            usecase::base_merge::BaseMergeService::execute(
                &interactor,
                usecase::base_merge::BaseMergeCommand { workspace_root: root.to_path_buf() },
            )
        };

        let first = run();
        assert!(matches!(
            first,
            Err(usecase::base_merge::BaseMergeError::PostMergeCleanup(
                usecase::base_merge::PostMergeCleanupError::SyncBaseStamp(
                    SyncBaseRecordError::Write(_)
                )
            ))
        ));
        assert!(root.join("track/.sotp-baseline-recovery/cleanup-test").is_dir());

        std::fs::remove_dir(&sync_stamp).unwrap();
        use std::io::Write as _;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&telemetry)
            .unwrap()
            .write_all(b"retry failure\n")
            .unwrap();

        let second = run();
        assert!(matches!(second, Ok(usecase::base_merge::BaseMergeOutcome::Completed)));
        assert_eq!(
            std::fs::read_to_string(&telemetry).unwrap(),
            "initial failure\nretry failure\n"
        );
        assert!(sync_stamp.is_file());
        assert!(!root.join("track/.sotp-baseline-recovery/cleanup-test").exists());
        assert!(!root.join("track/.sotp-baseline-recovery-cleanup-test").exists());
        let record = decode(&std::fs::read_to_string(sync_stamp).unwrap()).unwrap();
        assert_eq!(record.base_commit, base_commit);
    }

    #[test]
    fn test_fs_base_merge_cleanup_adapter_writes_v1_idempotently_and_replaces_later_commit() {
        let fixture = tempfile::tempdir().unwrap();
        let track_dir = fixture.path().join("track/items/cleanup-test");
        std::fs::create_dir_all(&track_dir).unwrap();
        let adapter = FsBaseMergeCleanupAdapter::new();
        let first = usecase::base_merge::BaseMergeCleanupRequest {
            workspace_root: fixture.path().to_path_buf(),
            track_id: TrackId::try_new("cleanup-test").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new("0123456789abcdef").unwrap(),
        };
        let later = usecase::base_merge::BaseMergeCleanupRequest {
            base_commit: CommitHash::try_new("fedcba9876543210").unwrap(),
            ..first.clone()
        };

        adapter.write_sync_base_record(&first).unwrap();
        adapter.write_sync_base_record(&first).unwrap();
        let first_record =
            decode(&std::fs::read_to_string(track_dir.join(".sync-base.json")).unwrap()).unwrap();
        assert_eq!(first_record.schema_version, SyncBaseRecordSchemaVersion::V1);
        assert_eq!(first_record.track_id, first.track_id);
        assert_eq!(first_record.base_branch, first.base_branch);
        assert_eq!(first_record.base_commit, first.base_commit);

        adapter.write_sync_base_record(&later).unwrap();

        let record =
            decode(&std::fs::read_to_string(track_dir.join(".sync-base.json")).unwrap()).unwrap();
        assert_eq!(record.base_commit, later.base_commit);
    }

    #[test]
    fn test_fs_base_merge_cleanup_adapter_conflict_keeps_track_available_without_cleanup() {
        let fixture = tempfile::tempdir().unwrap();
        let track_dir = fixture.path().join("track/items/cleanup-test");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("recovery-input.txt"), "preserved").unwrap();
        let interactor = usecase::base_merge::BaseMergeInteractor::new(
            Arc::new(FixedCleanupContext),
            Arc::new(ConflictedCleanupGit),
            Arc::new(FsBaseMergeCleanupAdapter::new()),
        );

        let outcome = usecase::base_merge::BaseMergeService::execute(
            &interactor,
            usecase::base_merge::BaseMergeCommand { workspace_root: fixture.path().to_path_buf() },
        )
        .unwrap();

        assert_eq!(outcome, usecase::base_merge::BaseMergeOutcome::Conflicted);
        assert!(track_dir.is_dir(), "the conflicted track must remain available for recovery");
        assert_eq!(
            std::fs::read_to_string(track_dir.join("recovery-input.txt")).unwrap(),
            "preserved"
        );
        assert!(!track_dir.join(".sync-base.json").exists());
        assert!(!track_dir.join("plan.md").exists());
        assert!(!track_dir.join("domain-types-baseline.json").exists());
    }

    #[test]
    fn test_fs_base_merge_cleanup_adapter_rejects_malformed_or_non_regular_stamp_without_replacing_prior_record()
     {
        let fixture = tempfile::tempdir().unwrap();
        let track_dir = fixture.path().join("track/items/cleanup-test");
        std::fs::create_dir_all(&track_dir).unwrap();
        let request = usecase::base_merge::BaseMergeCleanupRequest {
            workspace_root: fixture.path().to_path_buf(),
            track_id: TrackId::try_new("cleanup-test").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new("0123456789abcdef").unwrap(),
        };
        let adapter = FsBaseMergeCleanupAdapter::new();
        let stamp = track_dir.join(".sync-base.json");

        adapter.write_sync_base_record(&request).unwrap();
        let valid_record = std::fs::read(&stamp).unwrap();
        let temporary_replacement =
            track_dir.join(format!(".tmp-.sync-base.json-{}", std::process::id()));
        std::fs::create_dir(&temporary_replacement).unwrap();
        let later = usecase::base_merge::BaseMergeCleanupRequest {
            base_commit: CommitHash::try_new("fedcba9876543210").unwrap(),
            ..request.clone()
        };
        assert!(matches!(
            adapter.write_sync_base_record(&later),
            Err(SyncBaseRecordError::Replacement(_))
        ));
        assert_eq!(std::fs::read(&stamp).unwrap(), valid_record);
        std::fs::remove_dir(&temporary_replacement).unwrap();

        std::fs::write(&stamp, "{malformed").unwrap();
        let malformed = std::fs::read_to_string(&stamp).unwrap();
        assert!(matches!(
            adapter.write_sync_base_record(&request),
            Err(SyncBaseRecordError::Validation(_))
        ));
        assert_eq!(std::fs::read_to_string(&stamp).unwrap(), malformed);

        std::fs::remove_file(&stamp).unwrap();
        std::fs::create_dir(&stamp).unwrap();
        assert!(matches!(
            adapter.write_sync_base_record(&request),
            Err(SyncBaseRecordError::Write(_))
        ));
        assert!(stamp.is_dir(), "a non-regular stamp target must remain unchanged");
    }

    #[test]
    fn test_fs_base_merge_cleanup_adapter_rejects_invalid_isolated_fixture_and_preserves_prior_complete_track()
     {
        let fixture = tempfile::tempdir().unwrap();
        let track_dir = fixture.path().join("track/items/cleanup-test");
        std::fs::create_dir_all(&track_dir).unwrap();
        let prior = track_dir.join("domain-types-baseline.json");
        std::fs::write(&prior, "prior-valid-baseline").unwrap();
        std::fs::write(track_dir.join("track-input.json"), "prior-complete-track-input").unwrap();
        let request = usecase::base_merge::BaseMergeCleanupRequest {
            workspace_root: fixture.path().to_path_buf(),
            track_id: TrackId::try_new("cleanup-test").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new("0123456789abcdef").unwrap(),
        };

        let result = FsBaseMergeCleanupAdapter::new().replace_baselines(&request);

        assert!(matches!(result, Err(BaselineReplacementError::Isolation(_))));
        assert_eq!(std::fs::read_to_string(prior).unwrap(), "prior-valid-baseline");
        assert_eq!(
            std::fs::read_to_string(track_dir.join("track-input.json")).unwrap(),
            "prior-complete-track-input"
        );
        assert!(track_dir.is_dir(), "failed cleanup must not report a partial replacement");
    }

    #[test]
    fn test_fs_base_merge_cleanup_adapter_validation_failure_preserves_prior_complete_track() {
        let fixture = setup_cleanup_repository();
        let root = fixture.path();
        let track_dir = root.join("track/items/cleanup-test");
        let prior = track_dir.join("domain-types-baseline.json");
        std::fs::write(&prior, "prior-valid-baseline").unwrap();
        std::fs::write(track_dir.join("preserved-input.txt"), "prior-complete-track-input")
            .unwrap();
        let isolated_worktree = root.join("isolated-validation-worktree");
        let isolated_track = isolated_worktree.join("track/items/cleanup-test");
        std::fs::create_dir_all(&isolated_track).unwrap();
        std::fs::copy(
            root.join("architecture-rules.json"),
            isolated_worktree.join("architecture-rules.json"),
        )
        .unwrap();
        std::fs::copy(
            track_dir.join("tddd-features.json"),
            isolated_track.join("tddd-features.json"),
        )
        .unwrap();
        let malformed_generated_baseline = isolated_track.join("domain-types-baseline.json");
        std::fs::write(&malformed_generated_baseline, "{malformed").unwrap();

        let result = collect_validated_baselines(&isolated_worktree, "cleanup-test");

        assert!(result.is_err(), "malformed generated baselines must fail validation");
        assert_eq!(std::fs::read_to_string(&prior).unwrap(), "prior-valid-baseline");
        assert_eq!(
            std::fs::read_to_string(track_dir.join("preserved-input.txt")).unwrap(),
            "prior-complete-track-input"
        );
        assert!(track_dir.is_dir(), "validation failure must not expose a partial replacement");
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_base_merge_cleanup_adapter_publication_failure_preserves_prior_complete_track() {
        let fixture = tempfile::tempdir().unwrap();
        let items_dir = fixture.path().join("track/items");
        let track_dir = items_dir.join("cleanup-test");
        let replacement = items_dir.join(".sotp-baseline-replacement-test");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::create_dir(&replacement).unwrap();
        std::fs::write(track_dir.join(TRACK_WRITER_LOCK_FILE), b"").unwrap();
        std::fs::write(track_dir.join("domain-types-baseline.json"), "prior-valid-baseline")
            .unwrap();
        std::fs::write(track_dir.join("preserved-input.txt"), "prior-complete-track-input")
            .unwrap();
        let symlink_target = fixture.path().join("outside-replacement-input");
        std::fs::write(&symlink_target, "must-not-follow").unwrap();
        std::os::unix::fs::symlink(&symlink_target, replacement.join("invalid-baseline-input"))
            .unwrap();
        let _writer_lock = acquire_track_writer_lock(&track_dir, &items_dir).unwrap();

        let mut exchanged = false;
        let result = publish_baseline_replacements(
            &track_dir,
            &replacement,
            &BTreeSet::new(),
            &mut exchanged,
        );

        assert!(matches!(result, Err(BaselineReplacementError::Publish(_))));
        assert_eq!(
            std::fs::read_to_string(track_dir.join("domain-types-baseline.json")).unwrap(),
            "prior-valid-baseline"
        );
        assert_eq!(
            std::fs::read_to_string(track_dir.join("preserved-input.txt")).unwrap(),
            "prior-complete-track-input"
        );
        assert!(track_dir.is_dir(), "publication failure must not expose a partial replacement");
    }

    #[test]
    fn test_fs_base_merge_cleanup_adapter_restoration_failure_stays_typed_and_fails_closed() {
        let result = combine_baseline_cleanup_result(
            Err(BaselineReplacementError::Restoration {
                publish: DiagnosticText::new("atomic publication failed"),
                restoration: DiagnosticText::new("prior baseline restoration failed"),
            }),
            [Ok(()), Ok(()), Ok(())],
        );

        assert!(matches!(
            result,
            Err(BaselineReplacementError::Restoration { publish, restoration })
                if publish.as_str() == "atomic publication failed"
                    && restoration.as_str() == "prior baseline restoration failed"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_copy_cleanup_inputs_rejects_symlinked_architecture_rules() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let rules_target = source.path().join("untrusted-architecture-rules.json");
        std::fs::write(&rules_target, "{}").unwrap();
        std::os::unix::fs::symlink(&rules_target, source.path().join("architecture-rules.json"))
            .unwrap();

        let result = copy_cleanup_inputs(source.path(), target.path(), "cleanup-test");

        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_copy_cleanup_inputs_rejects_symlinked_detached_architecture_rules() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("architecture-rules.json"), "{}").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("architecture-rules.json"),
            target.path().join("architecture-rules.json"),
        )
        .unwrap();

        let result = copy_cleanup_inputs(source.path(), target.path(), "cleanup-test");

        assert!(result.is_err());
        assert!(!outside.path().join("architecture-rules.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_copy_cleanup_inputs_rejects_symlinked_detached_destination() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let source_track = source.path().join("track/items/cleanup-test");
        std::fs::create_dir_all(&source_track).unwrap();
        std::fs::write(source.path().join("architecture-rules.json"), "{}").unwrap();
        std::fs::write(source_track.join("current-input.json"), "current").unwrap();
        std::fs::create_dir_all(target.path().join("track")).unwrap();
        std::os::unix::fs::symlink(outside.path(), target.path().join("track/items")).unwrap();

        let result = copy_cleanup_inputs(source.path(), target.path(), "cleanup-test");

        assert!(result.is_err());
        assert!(!outside.path().join("cleanup-test/current-input.json").exists());
    }

    #[test]
    fn test_copy_cleanup_inputs_replaces_existing_detached_track_entries() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let source_track = source.path().join("track/items/cleanup-test");
        let target_track = target.path().join("track/items/cleanup-test");
        std::fs::create_dir_all(&source_track).unwrap();
        std::fs::create_dir_all(&target_track).unwrap();
        std::fs::write(
            source.path().join("architecture-rules.json"),
            r#"{
  "version": 2,
  "layers": [{
    "crate": "domain",
    "path": "libs/domain",
    "may_depend_on": [],
    "deny_reason": "",
    "verify": {"domain_purity": true, "domain_strings": true},
    "tddd": {
      "enabled": true,
      "catalogue_file": "shared.json",
      "schema_export": {"method": "rustdoc", "targets": ["domain"]}
    }
  }]
}"#,
        )
        .unwrap();
        std::fs::write(source_track.join("current-input.json"), "current").unwrap();
        std::fs::write(source_track.join("shared-baseline.json"), "stale-baseline").unwrap();
        std::fs::write(source_track.join("rogue-types-baseline.json"), "unconfigured").unwrap();
        std::fs::write(target_track.join("only-in-base-commit.json"), "stale").unwrap();

        copy_cleanup_inputs(source.path(), target.path(), "cleanup-test").unwrap();

        assert_eq!(
            std::fs::read_to_string(target_track.join("current-input.json")).unwrap(),
            "current"
        );
        assert!(!target_track.join("only-in-base-commit.json").exists());
        assert!(!target_track.join("shared-baseline.json").exists());
        assert_eq!(
            std::fs::read_to_string(target_track.join("rogue-types-baseline.json")).unwrap(),
            "unconfigured"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_base_merge_cleanup_rejects_symlinked_sync_stamp() {
        let fixture = tempfile::tempdir().unwrap();
        let track_dir = fixture.path().join("track/items/cleanup-test");
        std::fs::create_dir_all(&track_dir).unwrap();
        let stamp_target = fixture.path().join("untrusted-sync-base.json");
        std::fs::write(&stamp_target, "{}").unwrap();
        std::os::unix::fs::symlink(&stamp_target, track_dir.join(".sync-base.json")).unwrap();
        let request = usecase::base_merge::BaseMergeCleanupRequest {
            workspace_root: fixture.path().to_path_buf(),
            track_id: TrackId::try_new("cleanup-test").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new("0123456789abcdef").unwrap(),
        };

        let result = write_sync_base_record_atomically(&request);

        assert!(matches!(result, Err(SyncBaseRecordError::Validation(_))));
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_base_merge_cleanup_rejects_symlinked_items_root_for_sync_stamp() {
        let fixture = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(fixture.path().join("track")).unwrap();
        std::os::unix::fs::symlink(outside.path(), fixture.path().join("track/items")).unwrap();
        let request = usecase::base_merge::BaseMergeCleanupRequest {
            workspace_root: fixture.path().to_path_buf(),
            track_id: TrackId::try_new("cleanup-test").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new("0123456789abcdef").unwrap(),
        };

        let result = write_sync_base_record_atomically(&request);

        assert!(matches!(result, Err(SyncBaseRecordError::Validation(_))));
        assert!(!outside.path().join("cleanup-test/.sync-base.json").exists());
    }

    fn record() -> SyncBaseRecord {
        SyncBaseRecord {
            schema_version: SyncBaseRecordSchemaVersion::V1,
            track_id: TrackId::try_new("base-merge-track").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new("0123456789abcdef").unwrap(),
        }
    }

    #[test]
    fn test_sync_base_record_encode_decode_v1_preserves_validated_fields() {
        let value = record();

        let encoded = encode(&value).unwrap();

        assert_eq!(decode(&encoded).unwrap(), value);
        assert_eq!(
            encoded,
            r#"{"schema_version":"v1","track_id":"base-merge-track","base_branch":"develop","base_commit":"0123456789abcdef"}"#
        );
    }

    #[test]
    fn test_sync_base_record_decode_rejects_unknown_fields_and_invalid_domain_values() {
        for encoded in [
            r#"{"schema_version":"v1","track_id":"base-merge-track","base_branch":"develop","base_commit":"0123456789abcdef","extra":true}"#,
            r#"{"schema_version":"v1","track_id":"Not-a-track","base_branch":"develop","base_commit":"0123456789abcdef"}"#,
            r#"{"schema_version":"v1","track_id":"base-merge-track","base_branch":"-invalid","base_commit":"0123456789abcdef"}"#,
            r#"{"schema_version":"v1","track_id":"base-merge-track","base_branch":"develop","base_commit":"INVALID"}"#,
        ] {
            assert!(decode(encoded).is_err());
        }
    }

    #[test]
    fn test_sync_base_record_deserialize_rejects_unknown_schema_version() {
        let encoded = r#"{"schema_version":"v2","track_id":"base-merge-track","base_branch":"develop","base_commit":"0123456789abcdef"}"#;

        assert!(serde_json::from_str::<SyncBaseRecord>(encoded).is_err());
    }
}
