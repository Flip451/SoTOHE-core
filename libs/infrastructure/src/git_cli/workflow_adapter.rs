//! Infrastructure adapter implementing [`usecase::git_workflow::GitPrimitivePort`].
//!
//! [`FsGitWorkflowAdapter`] wraps [`SystemGitRepo`] to provide a concrete
//! implementation of the atomic git-primitive port. All infrastructure details
//! (process spawning, file I/O, git CLI) are kept inside the infrastructure
//! crate; the usecase-side `GitWorkflowInteractor` composes these primitives
//! with the pure branch-guard validators for the guarded git workflows.

use std::fs;
use std::path::{Component, Path, PathBuf};

use usecase::git_workflow::{
    DiagnosticText, ExplicitTrackBranch, GitPrimitivePort, GitWorkflowError,
    TRANSIENT_AUTOMATION_DIRS, TRANSIENT_AUTOMATION_FILES, TrackArchiveFsPort, TrackBranchClaim,
    validate_stage_path_entries,
};

use super::{SyncError, SystemGitRepo, collect_track_branch_claims, load_explicit_track_branch};

// ---------------------------------------------------------------------------
// Infrastructure adapter
// ---------------------------------------------------------------------------

/// Infrastructure adapter for guarded git workflow operations.
///
/// Implements [`GitPrimitivePort`] by delegating to [`SystemGitRepo`] and
/// the existing git_cli helper functions.
pub struct FsGitWorkflowAdapter;

impl FsGitWorkflowAdapter {
    /// Create a new `FsGitWorkflowAdapter`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsGitWorkflowAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// The former GitWorkflowService impl on FsGitWorkflowAdapter has been removed
// as part of the D8 flow census cutover — its responsibilities moved into
// usecase::git_workflow::GitWorkflowInteractor, which composes the atomic
// primitives exposed by the GitPrimitivePort impl (below) with the pure
// branch-guard validators.

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn resolve_repo_file_path(
    root: &Path,
    path: &Path,
    label: &str,
) -> Result<PathBuf, GitWorkflowError> {
    ensure_trusted_root(root)?;
    if path.as_os_str().is_empty() {
        return Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "{label} path must not be empty"
        ))));
    }
    if path.is_absolute() {
        return Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "{label} path must be repo-relative: {}",
            path.display()
        ))));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "{label} path cannot escape the repo root: {}",
            path.display()
        ))));
    }

    let resolved = root.join(path);
    if !resolved.starts_with(root) {
        return Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "{label} path must stay within repo root: {}",
            path.display()
        ))));
    }
    crate::track::symlink_guard::reject_symlinks_below(&resolved, root).map_err(|err| {
        GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "failed to validate {label} {}: {err}",
            resolved.display()
        )))
    })?;
    Ok(resolved)
}

fn resolve_repo_path_arg(
    root: &Path,
    path: &Path,
    label: &str,
) -> Result<String, GitWorkflowError> {
    ensure_trusted_root(root)?;
    if path.as_os_str().is_empty() {
        return Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "{label} path must not be empty"
        ))));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "{label} path cannot escape the repo root: {}",
            path.display()
        ))));
    }

    let repo_relative = if path.is_absolute() {
        path.strip_prefix(root).map_err(|_| {
            GitWorkflowError::Unavailable(DiagnosticText::new(format!(
                "{label} path must stay within repo root {}: {}",
                root.display(),
                path.display()
            )))
        })?
    } else {
        path
    };
    if repo_relative.as_os_str().is_empty() {
        return Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "{label} path must not be the repo root"
        ))));
    }

    let resolved = root.join(repo_relative);
    if !resolved.starts_with(root) {
        return Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "{label} path must stay within repo root {}: {}",
            root.display(),
            path.display()
        ))));
    }
    crate::track::symlink_guard::reject_symlinks_below(&resolved, root).map_err(|err| {
        GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "failed to validate {label} {}: {err}",
            resolved.display()
        )))
    })?;

    Ok(repo_relative.to_string_lossy().into_owned())
}

fn ensure_trusted_root(root: &Path) -> Result<(), GitWorkflowError> {
    match root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
                "refusing to use symlinked repository root: {}",
                root.display()
            ))))
        }
        Ok(_) => Ok(()),
        Err(err) => Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "failed to stat repository root {}: {err}",
            root.display()
        )))),
    }
}

fn ensure_existing_nonempty_file(path: &Path, label: &str) -> Result<(), GitWorkflowError> {
    if !path.is_file() {
        return Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "Missing {label}: {}",
            path.display()
        ))));
    }
    let content = fs::read_to_string(path).map_err(|err| {
        GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "failed to read {label} {}: {err}",
            path.display()
        )))
    })?;
    if content.trim().is_empty() {
        return Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "{label} is empty: {}",
            path.display()
        ))));
    }
    Ok(())
}

fn load_stage_paths(path: &Path) -> Result<Vec<String>, GitWorkflowError> {
    ensure_existing_nonempty_file(path, "stage path list file")?;
    let content = fs::read_to_string(path).map_err(|err| {
        GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "failed to read stage path list {}: {err}",
            path.display()
        )))
    })?;
    validate_stage_path_entries(content.lines()).map_err(|err| {
        let msg = err.to_string();
        if msg == "Stage path list file has no usable entries" {
            GitWorkflowError::Unavailable(DiagnosticText::new(format!("{msg}: {}", path.display())))
        } else {
            err
        }
    })
}

// ---------------------------------------------------------------------------
// GitPrimitivePort implementation
// ---------------------------------------------------------------------------

/// Discover a [`SystemGitRepo`] anchored to `project_root` (if given) or to
/// the process CWD.
fn discover_repo(project_root: Option<&Path>) -> Result<SystemGitRepo, GitWorkflowError> {
    match project_root {
        Some(root) => SystemGitRepo::discover_from(root)
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string()))),
        None => SystemGitRepo::discover()
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string()))),
    }
}

/// Map [`SyncError`] variants to [`GitWorkflowError`]. The three fail-closed
/// sync modes map 1:1 to the `GitWorkflowError::Sync*` variants; unavailable
/// git pull failures ([`SyncError::Spawn`]) map to
/// [`GitWorkflowError::Unavailable`]. IN-06 / CN-02 / AC-08.
fn sync_error_to_workflow(e: SyncError) -> GitWorkflowError {
    match e {
        SyncError::UpstreamNotSet => GitWorkflowError::SyncUpstreamNotSet,
        SyncError::NonFastForward { stderr } => GitWorkflowError::SyncNonFastForward { stderr },
        SyncError::WorktreeUnresolved { stderr } => {
            GitWorkflowError::SyncWorktreeUnresolved { stderr }
        }
        SyncError::Spawn { detail } => GitWorkflowError::Unavailable(detail),
    }
}

impl GitPrimitivePort for FsGitWorkflowAdapter {
    fn current_branch(
        &self,
        project_root: Option<&Path>,
    ) -> Result<Option<String>, GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        repo.current_branch()
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))
    }

    fn sync_current_branch(&self, project_root: Option<&Path>) -> Result<(), GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        repo.sync_current_branch().map_err(sync_error_to_workflow)
    }

    fn switch_branch(
        &self,
        project_root: Option<&Path>,
        branch: &str,
    ) -> Result<(), GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        let code = repo
            .status(&["switch", branch])
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))?;
        if code == 0 {
            Ok(())
        } else {
            Err(GitWorkflowError::SwitchFailed {
                branch: DiagnosticText::new(branch),
                exit_code: code,
            })
        }
    }

    fn create_branch(
        &self,
        project_root: Option<&Path>,
        new_branch: &str,
        base_branch: &str,
    ) -> Result<(), GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        let code = repo
            .status(&["switch", "-c", new_branch, base_branch])
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))?;
        if code == 0 {
            Ok(())
        } else {
            Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
                "git switch -c {new_branch} {base_branch} failed with exit code {code}"
            ))))
        }
    }

    fn branch_exists(
        &self,
        project_root: Option<&Path>,
        branch: &str,
    ) -> Result<bool, GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        let output = repo
            .output(&["rev-parse", "--verify", "--quiet", branch])
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))?;
        Ok(output.status.success())
    }

    fn move_path(
        &self,
        project_root: Option<&Path>,
        src: &Path,
        dst: &Path,
    ) -> Result<(), GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        let repo_root = repo.root().to_path_buf();
        let src_arg = resolve_repo_path_arg(&repo_root, src, "git mv source")?;
        let dst_arg = resolve_repo_path_arg(&repo_root, dst, "git mv destination")?;
        let output = repo
            .output(&["mv", &src_arg, &dst_arg])
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let code = output.status.code().unwrap_or(-1);
        Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
            "git mv failed (exit {code}): {stderr}"
        ))))
    }

    fn fetch_branch(
        &self,
        project_root: Option<&Path>,
        branch: &str,
    ) -> Result<(), GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        // Explicit refspec: on a single-branch or custom-refspec clone the
        // default `git fetch origin <branch>` does not refresh
        // `refs/remotes/origin/<branch>`, so downstream `git show
        // origin/<branch>:...` reads may see stale data. Force the
        // `+refs/heads/<branch>:refs/remotes/origin/<branch>` refspec to
        // preserve the pre-track baseline behavior.
        let refspec = format!("+refs/heads/{branch}:refs/remotes/origin/{branch}");
        let code = repo
            .status(&["fetch", "origin", &refspec])
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))?;
        if code == 0 {
            Ok(())
        } else {
            Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
                "git fetch origin {branch} failed with exit code {code}"
            ))))
        }
    }

    fn show_file_at_ref(
        &self,
        project_root: Option<&Path>,
        git_ref: &str,
        path: &Path,
    ) -> Result<String, GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        let path_str = path.to_string_lossy().into_owned();
        let ref_arg = format!("{git_ref}:{path_str}");
        let output = repo
            .output(&["show", &ref_arg])
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let code = output.status.code().unwrap_or(-1);
            return Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
                "git show {ref_arg} failed (exit {code}): {stderr}"
            ))));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn resolve_commit(
        &self,
        project_root: Option<&Path>,
        rev: &str,
    ) -> Result<Option<domain::CommitHash>, GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        let output = repo
            .output(&["rev-parse", "--verify", "--quiet", rev])
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))?;
        if !output.status.success() {
            return Ok(None);
        }
        let hash = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if hash.is_empty() {
            return Ok(None);
        }
        match domain::CommitHash::try_new(hash) {
            Ok(h) => Ok(Some(h)),
            Err(e) => Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
                "invalid commit hash for {rev}: {e}"
            )))),
        }
    }

    fn resolve_repo_root(&self, project_root: Option<&Path>) -> Result<PathBuf, GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        Ok(repo.root().to_path_buf())
    }

    fn stage_all(&self, project_root: Option<&Path>) -> Result<(), GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        repo.stage_all_excluding(TRANSIENT_AUTOMATION_FILES, TRANSIENT_AUTOMATION_DIRS)
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))
    }

    fn stage_from_file(
        &self,
        project_root: Option<&Path>,
        path: &Path,
        cleanup: bool,
    ) -> Result<(), GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        let resolved = resolve_repo_file_path(repo.root(), path, "stage path list file")?;

        let stage_paths = load_stage_paths(&resolved)?;

        let mut owned_args = vec!["add".to_owned(), "--".to_owned()];
        owned_args.extend(stage_paths);
        let args: Vec<&str> = owned_args.iter().map(String::as_str).collect();

        let code = repo
            .status(&args)
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))?;
        if code == 0 {
            if cleanup {
                let _ = fs::remove_file(&resolved);
            }
            Ok(())
        } else {
            Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
                "git add failed with exit code {code}"
            ))))
        }
    }

    fn commit_from_message_file(
        &self,
        project_root: Option<&Path>,
        path: &Path,
        cleanup: bool,
    ) -> Result<(), GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        let resolved = resolve_repo_file_path(repo.root(), path, "commit message file")?;
        ensure_existing_nonempty_file(&resolved, "commit message file")?;
        let path_str = resolved.to_string_lossy().into_owned();
        let code = repo
            .status(&["commit", "-F", path_str.as_str()])
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))?;
        if code == 0 {
            if cleanup {
                let _ = fs::remove_file(&resolved);
            }
            Ok(())
        } else {
            Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
                "git commit failed with exit code {code}"
            ))))
        }
    }

    fn note_from_file(
        &self,
        project_root: Option<&Path>,
        path: &Path,
        cleanup: bool,
    ) -> Result<(), GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        let resolved = resolve_repo_file_path(repo.root(), path, "git note file")?;
        ensure_existing_nonempty_file(&resolved, "git note file")?;
        let path_str = resolved.to_string_lossy().into_owned();
        let code = repo
            .status(&["notes", "add", "-f", "-F", path_str.as_str(), "HEAD"])
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))?;
        if code == 0 {
            if cleanup {
                let _ = fs::remove_file(&resolved);
            }
            Ok(())
        } else {
            Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
                "git notes add failed with exit code {code}"
            ))))
        }
    }

    fn unstage(
        &self,
        project_root: Option<&Path>,
        paths: &[PathBuf],
    ) -> Result<(), GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        let mut args = vec!["restore", "--staged", "--"];
        let path_strs: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        args.extend(path_strs.iter().map(String::as_str));
        let code = repo
            .status(&args)
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))?;
        if code == 0 {
            Ok(())
        } else {
            Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
                "git restore --staged failed with exit code {code}"
            ))))
        }
    }

    fn read_explicit_track_branch(
        &self,
        project_root: Option<&Path>,
        track_dir: &Path,
    ) -> Result<ExplicitTrackBranch, GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        let resolved_td = resolve_repo_file_path(repo.root(), track_dir, "track directory path")?;
        load_explicit_track_branch(repo.root(), &resolved_td)
            .map(|record| ExplicitTrackBranch {
                display_path: record.display_path,
                expected_branch: record.branch,
                status: record.status,
            })
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))
    }

    fn collect_track_branch_claims(
        &self,
        project_root: Option<&Path>,
    ) -> Result<Vec<TrackBranchClaim>, GitWorkflowError> {
        let repo = discover_repo(project_root)?;
        collect_track_branch_claims(repo.root())
            .map(|records| {
                records
                    .into_iter()
                    .map(|record| TrackBranchClaim {
                        track_name: record.track_name,
                        branch: record.branch,
                        status: record.status,
                    })
                    .collect()
            })
            .map_err(|e| GitWorkflowError::Unavailable(DiagnosticText::new(e.to_string())))
    }
}

// ---------------------------------------------------------------------------
// FsWorkspaceAdapter — TrackArchiveFsPort implementation
// ---------------------------------------------------------------------------

/// Adapter implementing [`TrackArchiveFsPort`] over `std::fs`. Failures map
/// to [`GitWorkflowError::Fs`]. Unit struct. IN-08 / IN-10 / AC-09.
#[derive(Default)]
pub struct FsWorkspaceAdapter;

impl FsWorkspaceAdapter {
    /// Unit-struct constructor.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

fn fs_error(action: &str, path: &Path, source: &dyn std::fmt::Display) -> GitWorkflowError {
    GitWorkflowError::Fs {
        detail: DiagnosticText::new(format!("{action} {}: {source}", path.display())),
    }
}

fn reject_symlinked_fs_path(path: &Path, label: &str) -> Result<(), GitWorkflowError> {
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let cwd = std::env::current_dir()
            .map_err(|err| fs_error("failed to resolve current directory for", path, &err))?;
        cwd.join(path)
    };

    let mut ancestors: Vec<PathBuf> = absolute_path.ancestors().map(Path::to_path_buf).collect();
    ancestors.reverse();

    for ancestor in ancestors {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        match ancestor.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(GitWorkflowError::Fs {
                    detail: DiagnosticText::new(format!(
                        "refusing to follow symlink in {label}: {}",
                        ancestor.display()
                    )),
                });
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(fs_error("failed to stat", &ancestor, &err)),
        }
    }

    Ok(())
}

impl TrackArchiveFsPort for FsWorkspaceAdapter {
    fn path_is_dir(&self, path: &Path) -> Result<bool, GitWorkflowError> {
        reject_symlinked_fs_path(path, "directory path")?;
        match path.symlink_metadata() {
            Ok(meta) => Ok(meta.file_type().is_dir()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(fs_error("failed to stat", path, &err)),
        }
    }

    fn path_exists(&self, path: &Path) -> Result<bool, GitWorkflowError> {
        reject_symlinked_fs_path(path, "filesystem path")?;
        match path.symlink_metadata() {
            Ok(_) => Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(fs_error("failed to stat", path, &err)),
        }
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), GitWorkflowError> {
        reject_symlinked_fs_path(path, "directory creation path")?;
        fs::create_dir_all(path).map_err(|err| fs_error("failed to create directory", path, &err))
    }

    fn rename_path(&self, src: &Path, dst: &Path) -> Result<(), GitWorkflowError> {
        reject_symlinked_fs_path(src, "rename source path")?;
        reject_symlinked_fs_path(dst, "rename destination path")?;
        fs::rename(src, dst).map_err(|err| GitWorkflowError::Fs {
            detail: DiagnosticText::new(format!(
                "failed to rename {} to {}: {err}",
                src.display(),
                dst.display()
            )),
        })
    }

    fn list_dir_file_names(&self, path: &Path) -> Result<Vec<PathBuf>, GitWorkflowError> {
        reject_symlinked_fs_path(path, "directory read path")?;

        let mut entries = Vec::new();
        let read_dir =
            fs::read_dir(path).map_err(|err| fs_error("failed to read directory", path, &err))?;
        for entry in read_dir {
            let entry =
                entry.map_err(|err| fs_error("failed to read directory entry in", path, &err))?;
            entries.push(entry.path());
        }
        entries.sort();
        Ok(entries)
    }

    fn remove_dir(&self, path: &Path) -> Result<(), GitWorkflowError> {
        reject_symlinked_fs_path(path, "directory removal path")?;
        fs::remove_dir(path).map_err(|err| fs_error("failed to remove directory", path, &err))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::Path;

    use usecase::git_workflow::{
        DiagnosticText, GitPrimitivePort as _, GitWorkflowError, TrackArchiveFsPort as _,
    };

    use super::{
        FsGitWorkflowAdapter, FsWorkspaceAdapter, resolve_repo_file_path, resolve_repo_path_arg,
        sync_error_to_workflow,
    };

    use crate::git_cli::SyncError;
    use crate::verify::test_support::run_git;

    #[cfg(unix)]
    #[test]
    fn resolve_repo_file_path_rejects_symlinked_root() {
        let real_root = tempfile::tempdir().unwrap();
        let link_parent = tempfile::tempdir().unwrap();
        let root_link = link_parent.path().join("workspace-link");
        std::os::unix::fs::symlink(real_root.path(), &root_link).unwrap();

        let err =
            resolve_repo_file_path(&root_link, Path::new("add-paths.txt"), "stage path list file")
                .unwrap_err();

        assert!(err.to_string().contains("refusing to use symlinked repository root"), "{err}");
    }

    #[test]
    fn resolve_repo_path_arg_converts_absolute_repo_path_to_relative_arg() {
        let root = tempfile::tempdir().unwrap();
        let src = root.path().join("track/items/example");

        let arg = resolve_repo_path_arg(root.path(), &src, "git mv source").unwrap();

        assert_eq!(arg, "track/items/example");
    }

    #[test]
    fn resolve_repo_path_arg_rejects_absolute_path_outside_repo() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let src = outside.path().join("track/items/example");

        let err = resolve_repo_path_arg(root.path(), &src, "git mv source").unwrap_err();

        assert!(err.to_string().contains("must stay within repo root"), "{err}");
    }

    #[test]
    fn resolve_repo_path_arg_rejects_parent_dir_escape() {
        let root = tempfile::tempdir().unwrap();

        let err = resolve_repo_path_arg(root.path(), Path::new("../outside"), "git mv source")
            .unwrap_err();

        assert!(err.to_string().contains("cannot escape the repo root"), "{err}");
    }

    // ── GitPrimitivePort::sync_current_branch ──────────────────────────────

    /// A brand-new local repo with a commit but no upstream tracking must
    /// map git's "no tracking information" stderr onto
    /// [`GitWorkflowError::SyncUpstreamNotSet`] via [`SyncError::UpstreamNotSet`].
    #[test]
    fn sync_current_branch_maps_missing_upstream_to_upstream_not_set() {
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init", "-b", "main"]);
        run_git(dir.path(), &["config", "user.email", "test@example.com"]);
        run_git(dir.path(), &["config", "user.name", "Test"]);
        run_git(dir.path(), &["commit", "--allow-empty", "-m", "init"]);

        let adapter = FsGitWorkflowAdapter::new();

        let err = adapter.sync_current_branch(Some(dir.path())).unwrap_err();
        assert!(
            matches!(err, GitWorkflowError::SyncUpstreamNotSet),
            "expected SyncUpstreamNotSet, got: {err:?}"
        );
    }

    #[test]
    fn test_sync_error_to_workflow_non_fast_forward_maps_to_sync_non_fast_forward() {
        let detail = "fatal: Not possible to fast-forward, aborting.";

        let err = sync_error_to_workflow(SyncError::NonFastForward {
            stderr: DiagnosticText::new(detail),
        });

        assert!(
            matches!(err, GitWorkflowError::SyncNonFastForward { .. }),
            "expected SyncNonFastForward, got: {err:?}"
        );
        if let GitWorkflowError::SyncNonFastForward { stderr } = err {
            assert_eq!(stderr.as_str(), detail);
        }
    }

    #[test]
    fn test_sync_error_to_workflow_worktree_unresolved_maps_to_sync_worktree_unresolved() {
        let detail =
            "error: Your local changes to the following files would be overwritten by merge:";

        let err = sync_error_to_workflow(SyncError::WorktreeUnresolved {
            stderr: DiagnosticText::new(detail),
        });

        assert!(
            matches!(err, GitWorkflowError::SyncWorktreeUnresolved { .. }),
            "expected SyncWorktreeUnresolved, got: {err:?}"
        );
        if let GitWorkflowError::SyncWorktreeUnresolved { stderr } = err {
            assert_eq!(stderr.as_str(), detail);
        }
    }

    /// [`GitPrimitivePort::current_branch`] anchored to a temp repo returns
    /// the current branch name — verifies the port impl actually delegates
    /// to the underlying `SystemGitRepo`.
    #[test]
    fn git_primitive_current_branch_returns_current_branch_name() {
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init", "-b", "main"]);
        run_git(dir.path(), &["config", "user.email", "test@example.com"]);
        run_git(dir.path(), &["config", "user.name", "Test"]);
        run_git(dir.path(), &["commit", "--allow-empty", "-m", "init"]);
        run_git(dir.path(), &["checkout", "-b", "track/example"]);

        let adapter = FsGitWorkflowAdapter::new();

        assert_eq!(
            adapter.current_branch(Some(dir.path())).unwrap().as_deref(),
            Some("track/example")
        );
    }

    // ── FsWorkspaceAdapter (TrackArchiveFsPort) ────────────────────────────

    #[test]
    fn fs_workspace_path_exists_and_is_dir_reflect_filesystem_state() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("child");
        std::fs::create_dir_all(&existing).unwrap();
        let missing = dir.path().join("missing");

        let fs = FsWorkspaceAdapter::new();
        assert!(fs.path_exists(&existing).unwrap());
        assert!(fs.path_is_dir(&existing).unwrap());
        assert!(!fs.path_exists(&missing).unwrap());
        assert!(!fs.path_is_dir(&missing).unwrap());
    }

    #[test]
    fn fs_workspace_create_dir_all_creates_intermediate_directories() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("a/b/c");

        let fs = FsWorkspaceAdapter::new();
        fs.create_dir_all(&target).unwrap();

        assert!(target.is_dir());
    }

    #[test]
    fn fs_workspace_rename_path_moves_source_to_destination() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();

        let fs = FsWorkspaceAdapter::new();
        fs.rename_path(&src, &dst).unwrap();

        assert!(!src.exists());
        assert!(dst.is_dir());
    }

    #[test]
    fn fs_workspace_rename_path_missing_source_maps_to_fs_error() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("missing");
        let dst = dir.path().join("dst");

        let fs = FsWorkspaceAdapter::new();
        let err = fs.rename_path(&src, &dst).unwrap_err();
        assert!(matches!(err, GitWorkflowError::Fs { .. }));
    }

    #[test]
    fn fs_workspace_list_dir_file_names_returns_sorted_entries() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("dir");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("b.txt"), "b").unwrap();
        std::fs::write(sub.join("a.txt"), "a").unwrap();

        let fs = FsWorkspaceAdapter::new();
        let mut entries = fs.list_dir_file_names(&sub).unwrap();
        entries.sort();

        assert_eq!(entries.len(), 2);
        assert!(entries.first().is_some_and(|entry| entry.ends_with("a.txt")));
        assert!(entries.get(1).is_some_and(|entry| entry.ends_with("b.txt")));
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_workspace_list_dir_file_names_symlinked_directory_returns_fs_error() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let link = dir.path().join("link");
        std::fs::create_dir_all(&target).unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let fs = FsWorkspaceAdapter::new();
        let err = fs.list_dir_file_names(&link).unwrap_err();

        assert!(matches!(err, GitWorkflowError::Fs { .. }));
        assert!(err.to_string().contains("refusing to follow symlink"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_workspace_create_dir_all_symlinked_parent_returns_fs_error() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let link = dir.path().join("link");
        std::fs::create_dir_all(&target).unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let fs = FsWorkspaceAdapter::new();
        let err = fs.create_dir_all(&link.join("child")).unwrap_err();

        assert!(matches!(err, GitWorkflowError::Fs { .. }));
        assert!(err.to_string().contains("refusing to follow symlink"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_workspace_rename_path_symlinked_destination_parent_returns_fs_error() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let target = dir.path().join("target");
        let link = dir.path().join("link");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let fs = FsWorkspaceAdapter::new();
        let err = fs.rename_path(&src, &link.join("dst")).unwrap_err();

        assert!(matches!(err, GitWorkflowError::Fs { .. }));
        assert!(err.to_string().contains("refusing to follow symlink"), "{err}");
        assert!(src.exists());
    }

    #[test]
    fn fs_workspace_remove_dir_removes_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("empty");
        std::fs::create_dir_all(&sub).unwrap();

        let fs = FsWorkspaceAdapter::new();
        fs.remove_dir(&sub).unwrap();

        assert!(!sub.exists());
    }
}
