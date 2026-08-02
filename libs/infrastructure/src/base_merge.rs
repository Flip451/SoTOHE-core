//! Filesystem and git adapters for guarded base merges, plus their persistence codec.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use domain::branch_strategy::{BaseBranchName, BaseMergeDirection, derive_base_merge_direction};
use domain::{CommitHash, TrackBranch, TrackId};
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
use crate::track::atomic_write::atomic_write_file;
use crate::track::render::sync_rendered_views;
use crate::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};

const MAX_BASE_MERGE_GIT_OUTPUT_BYTES: usize = 8 * 1024;
const MAX_CLEANUP_TREE_DEPTH: usize = 64;
const MAX_CLEANUP_TREE_ENTRIES: usize = 10_000;
const MAX_CLEANUP_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CLEANUP_TREE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_SYNC_BASE_RECORD_BYTES: u64 = 64 * 1024;

mod cleanup_tree;
mod sync_base_record;

use cleanup_tree::{
    capture_baselines_in_worktree, collect_validated_baselines, copy_cleanup_inputs,
    remove_tree_bounded, replace_tree, sync_tree,
};
pub use sync_base_record::{SyncBaseRecord, SyncBaseRecordSchemaVersion};
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
        let current = read_current_track_branch(&repository_root).map_err(git_execution_error)?;
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

        let base_commit = resolve_base_commit(&repository_root, authoritative_direction.source())?;
        let output = match run_guarded_merge(&repository_root, &base_commit) {
            Ok(output) => output,
            Err(_) => return adjudicate_merge_after_runner_error(&repository_root, base_commit),
        };
        if output.status.success() {
            return Ok(BaseMergeAttemptOutcome::Clean { base_commit });
        }

        if has_unmerged_paths(&repository_root)? {
            return Ok(BaseMergeAttemptOutcome::Conflicted);
        }

        Err(git_execution_error("guarded git merge failed"))
    }
}

/// Filesystem-backed implementation of the ordered clean-merge cleanup.
///
/// The adapter deliberately keeps the three stages separate at the port
/// boundary.  Baselines are generated in a detached worktree at the exact
/// commit supplied by the merge port; publication happens only after every
/// generated file has passed rustdoc validation.  The sync stamp is written
/// last and never re-resolves the source branch.
pub struct FsBaseMergeCleanupAdapter;

#[allow(clippy::new_without_default)]
impl FsBaseMergeCleanupAdapter {
    /// Creates the filesystem-backed cleanup adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
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
        replace_baselines_from_exact_commit(request)
    }

    fn write_sync_base_record(
        &self,
        request: &BaseMergeCleanupRequest,
    ) -> Result<(), SyncBaseRecordError> {
        write_sync_base_record_atomically(request)
    }
}

fn replace_baselines_from_exact_commit(
    request: &BaseMergeCleanupRequest,
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

    let worktree = create_unique_directory(&repository_root, ".sotp-base-merge-worktree-")
        .map_err(|error| {
            BaselineReplacementError::Isolation(DiagnosticText::new(error.to_string()))
        })?;
    let track_parent = track_dir.parent().ok_or_else(|| {
        BaselineReplacementError::Isolation(DiagnosticText::new(
            "active track directory has no parent directory",
        ))
    })?;
    let replacement = match create_unique_directory(track_parent, ".sotp-baseline-replacement-") {
        Ok(replacement) => replacement,
        Err(error) => {
            let detail = cleanup_directory(&worktree, "detached cleanup worktree")
                .err()
                .map(|cleanup| format!("; cleanup also failed: {cleanup}"))
                .unwrap_or_default();
            return Err(BaselineReplacementError::Isolation(DiagnosticText::new(format!(
                "cannot create baseline replacement directory: {error}{detail}"
            ))));
        }
    };

    let result: Result<(), BaselineReplacementError> = (|| {
        add_commit_pinned_worktree(&repository_root, &worktree, &request.base_commit)
            .map_err(|error| BaselineReplacementError::Isolation(DiagnosticText::new(error)))?;
        copy_cleanup_inputs(&request.workspace_root, &worktree, request.track_id.as_ref())
            .map_err(|error| BaselineReplacementError::Isolation(DiagnosticText::new(error)))?;
        capture_baselines_in_worktree(&worktree, request.track_id.as_ref())?;
        collect_validated_baselines(&worktree, request.track_id.as_ref())
            .map_err(|error| BaselineReplacementError::Validation(DiagnosticText::new(error)))?;
        replace_tree(
            &worktree.join("track/items").join(request.track_id.as_ref()),
            &replacement,
            true,
            track_parent,
        )
        .map_err(|error| BaselineReplacementError::Validation(DiagnosticText::new(error)))?;
        publish_baseline_replacements(&track_dir, &replacement)
    })();

    let removal = remove_commit_pinned_worktree(&repository_root, &worktree)
        .map_err(|error| format!("cannot unregister detached cleanup worktree: {error}"));
    let directory_cleanup = cleanup_directory(&worktree, "detached cleanup worktree");
    // A publication error may have occurred only after the atomic exchange,
    // leaving `replacement` as the complete prior track for recovery.
    let replacement_cleanup =
        if result.is_err() && !matches!(&result, Err(BaselineReplacementError::Publish(_))) {
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
fn publish_baseline_replacements(
    track_dir: &Path,
    replacement: &Path,
) -> Result<(), BaselineReplacementError> {
    let track_parent = track_dir.parent().ok_or_else(|| {
        BaselineReplacementError::Publish(DiagnosticText::new(
            "active track directory has no parent directory",
        ))
    })?;
    if replacement.parent() != Some(track_parent) {
        return Err(BaselineReplacementError::Publish(DiagnosticText::new(
            "baseline replacement is not a sibling of the active track directory",
        )));
    }
    let track_name = track_dir.file_name().ok_or_else(|| {
        BaselineReplacementError::Publish(DiagnosticText::new(
            "active track directory has no directory name",
        ))
    })?;
    let replacement_name = replacement.file_name().ok_or_else(|| {
        BaselineReplacementError::Publish(DiagnosticText::new(
            "baseline replacement directory has no directory name",
        ))
    })?;
    let parent_dir = fs::File::open(track_parent).map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot open active track parent directory {}: {error}",
            track_parent.display()
        )))
    })?;

    sync_tree(replacement, track_parent).map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot make staged baseline replacement durable: {error}"
        )))
    })?;

    rustix::fs::renameat_with(
        &parent_dir,
        track_name,
        &parent_dir,
        replacement_name,
        rustix::fs::RenameFlags::EXCHANGE,
    )
    .map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot atomically publish complete baseline replacement: {error}"
        )))
    })?;

    parent_dir.sync_all().map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "published baseline replacement but cannot persist directory exchange: {error}"
        )))
    })?;

    // After the atomic exchange `replacement` holds the complete prior track.
    // A failed cleanup leaves it intact for recovery without exposing a
    // partial live track.
    if let Err(error) = remove_tree_bounded(replacement, track_parent) {
        return Err(BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "published baseline replacement but cannot remove recoverable prior track {}: {error}",
            replacement.display()
        ))));
    }
    parent_dir.sync_all().map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "published baseline replacement but cannot persist prior-directory cleanup: {error}"
        )))
    })?;
    Ok(())
}

fn write_sync_base_record_atomically(
    request: &BaseMergeCleanupRequest,
) -> Result<(), SyncBaseRecordError> {
    let track_dir = request.workspace_root.join("track/items").join(request.track_id.as_ref());
    let items_dir = request.workspace_root.join("track/items");
    reject_symlinks_up_to_root(&items_dir)
        .map_err(|error| SyncBaseRecordError::Validation(DiagnosticText::new(error.to_string())))?;
    reject_symlinks_below(&track_dir, &items_dir)
        .map_err(|error| SyncBaseRecordError::Validation(DiagnosticText::new(error.to_string())))?;
    if !track_dir.is_dir() {
        return Err(SyncBaseRecordError::Write(DiagnosticText::new(
            "active track directory is unavailable",
        )));
    }

    let record = SyncBaseRecord {
        schema_version: SyncBaseRecordSchemaVersion::V1,
        track_id: request.track_id.clone(),
        base_branch: request.base_branch.clone(),
        base_commit: request.base_commit.clone(),
    };
    let encoded = encode(&record)
        .map_err(|error| SyncBaseRecordError::Generation(DiagnosticText::new(error.to_string())))?;
    let decoded = decode(&encoded)
        .map_err(|error| SyncBaseRecordError::Validation(DiagnosticText::new(error.to_string())))?;
    if decoded != record {
        return Err(SyncBaseRecordError::Validation(DiagnosticText::new(
            "sync-base record failed round-trip validation",
        )));
    }

    let path = track_dir.join(".sync-base.json");
    if reject_symlinks_below(&path, &track_dir)
        .map_err(|error| SyncBaseRecordError::Validation(DiagnosticText::new(error.to_string())))?
    {
        let existing = read_regular_file_bounded(&path, &track_dir, MAX_SYNC_BASE_RECORD_BYTES)
            .map_err(|error| SyncBaseRecordError::Write(DiagnosticText::new(error)))?;
        let existing = std::str::from_utf8(&existing).map_err(|error| {
            SyncBaseRecordError::Validation(DiagnosticText::new(format!(
                "existing sync-base record is not UTF-8: {error}"
            )))
        })?;
        match decode(existing) {
            Ok(previous) if previous == record => return Ok(()),
            Err(error) => {
                return Err(SyncBaseRecordError::Validation(DiagnosticText::new(
                    error.to_string(),
                )));
            }
            Ok(_) => {}
        }
    }

    atomic_write_file(&path, encoded.as_bytes())
        .map_err(|error| SyncBaseRecordError::Replacement(DiagnosticText::new(error.to_string())))
}

fn read_regular_file_bounded(
    path: &Path,
    trusted_root: &Path,
    limit: u64,
) -> Result<Vec<u8>, String> {
    reject_symlinks_below(path, trusted_root)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("refusing symlinked file: {}", path.display()));
    }
    if !metadata.is_file() {
        return Err(format!("refusing non-regular file: {}", path.display()));
    }
    if metadata.len() > limit {
        return Err(format!("file exceeds read-size limit: {}", path.display()));
    }
    let mut file =
        fs::File::open(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect opened file {}: {error}", path.display()))?;
    if !opened_metadata.is_file() {
        return Err(format!("refusing non-regular file: {}", path.display()));
    }
    let mut content = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    Read::by_ref(&mut file)
        .take(limit.saturating_add(1))
        .read_to_end(&mut content)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if u64::try_from(content.len()).map_or(true, |length| length > limit) {
        return Err(format!("file exceeds read-size limit: {}", path.display()));
    }
    Ok(content)
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
    let revision = format!("{}^{{commit}}", source.as_str());
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
        return Ok(BaseMergeAttemptOutcome::Conflicted);
    }
    if merge_head_is_present(repository_root)? {
        return Err(git_execution_error("guarded git merge ended with an unresolved merge state"));
    }
    if base_commit_is_merged_into_head(repository_root, &base_commit)? {
        return Ok(BaseMergeAttemptOutcome::Clean { base_commit });
    }
    Err(git_execution_error("guarded git merge could not be adjudicated after runner failure"))
}

fn merge_head_is_present(repository_root: &Path) -> Result<bool, BaseMergeGitError> {
    let output = isolated_bounded_git_output(
        repository_root,
        &["rev-parse", "--verify", "--quiet", "MERGE_HEAD^{commit}"],
        MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    )
    .map_err(|_| git_execution_error("merge state could not be inspected"))?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(git_execution_error("merge state could not be inspected")),
    }
}

fn base_commit_is_merged_into_head(
    repository_root: &Path,
    base_commit: &CommitHash,
) -> Result<bool, BaseMergeGitError> {
    let output = isolated_bounded_git_output(
        repository_root,
        &["merge-base", "--is-ancestor", base_commit.as_ref(), "HEAD"],
        MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    )
    .map_err(|_| git_execution_error("merged HEAD could not be inspected"))?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(git_execution_error("merged HEAD could not be inspected")),
    }
}

fn has_unmerged_paths(repository_root: &Path) -> Result<bool, BaseMergeGitError> {
    let output = isolated_bounded_git_output(
        repository_root,
        &["diff", "--quiet", "--diff-filter=U", "--"],
        1,
    )
    .map_err(|_| git_execution_error("merge conflict state could not be inspected"))?;
    match output.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(git_execution_error("merge conflict state could not be inspected")),
    }
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
    use std::path::Path;

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

    fn current_commit(root: &Path, revision: &str) -> String {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--verify", revision])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(output.status.success(), "revision must resolve: {revision}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
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
        let replacement =
            create_unique_directory(root.join("track/items").as_path(), ".test-replacement-")
                .unwrap();
        let baseline = track_dir.join("domain-types-baseline.json");
        let stale_baseline = track_dir.join("obsolete-types-baseline.json");
        let type_signals = track_dir.join("domain-type-signals.json");
        std::fs::write(&baseline, "prior-valid-baseline").unwrap();
        std::fs::write(&stale_baseline, "obsolete-baseline").unwrap();
        std::fs::write(&type_signals, "preserved-cache").unwrap();
        replace_tree(&track_dir, &replacement, true, root.join("track/items").as_path()).unwrap();
        std::fs::write(replacement.join("domain-types-baseline.json"), "replacement-baseline")
            .unwrap();
        std::fs::remove_file(replacement.join("obsolete-types-baseline.json")).unwrap();

        publish_baseline_replacements(&track_dir, &replacement).unwrap();

        assert_eq!(std::fs::read_to_string(&baseline).unwrap(), "replacement-baseline");
        assert!(!stale_baseline.exists(), "stale baseline must be removed from the replacement");
        assert_eq!(std::fs::read_to_string(&type_signals).unwrap(), "preserved-cache");
        assert!(!replacement.exists(), "replacement directory must be consumed atomically");
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
        remove_commit_pinned_worktree(root, &worktree).unwrap();
        let _ = std::fs::remove_dir_all(&worktree);
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

        write_sync_base_record_atomically(&request).unwrap();
        write_sync_base_record_atomically(&request).unwrap();

        let encoded = std::fs::read_to_string(track_dir.join(".sync-base.json")).unwrap();
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded.schema_version, SyncBaseRecordSchemaVersion::V1);
        assert_eq!(decoded.track_id, request.track_id);
        assert_eq!(decoded.base_branch, request.base_branch);
        assert_eq!(decoded.base_commit, request.base_commit);
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
        std::fs::write(source.path().join("architecture-rules.json"), "{}").unwrap();
        std::fs::write(source_track.join("current-input.json"), "current").unwrap();
        std::fs::write(source_track.join("domain-types-baseline.json"), "stale-baseline").unwrap();
        std::fs::write(target_track.join("only-in-base-commit.json"), "stale").unwrap();

        copy_cleanup_inputs(source.path(), target.path(), "cleanup-test").unwrap();

        assert_eq!(
            std::fs::read_to_string(target_track.join("current-input.json")).unwrap(),
            "current"
        );
        assert!(!target_track.join("only-in-base-commit.json").exists());
        assert!(!target_track.join("domain-types-baseline.json").exists());
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
