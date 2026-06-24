//! Infrastructure adapter implementing [`usecase::git_workflow::GitWorkflowService`].
//!
//! [`FsGitWorkflowAdapter`] wraps [`SystemGitRepo`] and the helper functions
//! from this module to provide a concrete implementation of the service trait.
//! This keeps all infrastructure details (process spawning, file I/O, git CLI)
//! inside the infrastructure crate.

use std::fs;
use std::path::{Component, Path, PathBuf};

use usecase::git_workflow::{
    ExplicitTrackBranch, GitWorkflowError, GitWorkflowResult, GitWorkflowService,
    TRANSIENT_AUTOMATION_DIRS, TRANSIENT_AUTOMATION_FILES, TrackBranchClaim,
    validate_stage_path_entries, verify_auto_detected_branch, verify_explicit_track_branch,
};
use usecase::track_resolution;

use super::{
    GitRepository as _, SystemGitRepo, collect_track_branch_claims, load_explicit_track_branch,
};

// ---------------------------------------------------------------------------
// Infrastructure adapter
// ---------------------------------------------------------------------------

/// Infrastructure adapter for guarded git workflow operations.
///
/// Implements [`GitWorkflowService`] by delegating to [`SystemGitRepo`] and
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

impl GitWorkflowService for FsGitWorkflowAdapter {
    fn stage_all(&self) -> GitWorkflowResult<()> {
        let repo =
            SystemGitRepo::discover().map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?;
        repo.stage_all_excluding(TRANSIENT_AUTOMATION_FILES, TRANSIENT_AUTOMATION_DIRS)
            .map_err(|e| GitWorkflowError::Unavailable(e.to_string()))
    }

    fn stage_from_file(&self, path: &Path, cleanup: bool) -> GitWorkflowResult<()> {
        let repo =
            SystemGitRepo::discover().map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?;
        let resolved = resolve_repo_file_path(repo.root(), path, "stage path list file")?;

        let stage_paths = load_stage_paths(&resolved)?;

        let mut owned_args = vec!["add".to_owned(), "--".to_owned()];
        owned_args.extend(stage_paths);
        let args: Vec<&str> = owned_args.iter().map(String::as_str).collect();

        let code = repo.status(&args).map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?;
        if code == 0 {
            if cleanup {
                let _ = fs::remove_file(&resolved);
            }
            Ok(())
        } else {
            Err(GitWorkflowError::Unavailable(format!("git add failed with exit code {code}")))
        }
    }

    fn commit_from_file(
        &self,
        path: &Path,
        cleanup: bool,
        track_dir: Option<&Path>,
    ) -> GitWorkflowResult<()> {
        let repo =
            SystemGitRepo::discover().map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?;
        let resolved = resolve_repo_file_path(repo.root(), path, "commit message file")?;

        ensure_existing_nonempty_file(&resolved, "commit message file")?;

        let explicit_track = track_dir
            .map(|td| {
                let resolved_td = resolve_repo_file_path(repo.root(), td, "track directory path")?;
                load_explicit_track_branch(repo.root(), &resolved_td)
                    .map_err(|e| GitWorkflowError::Unavailable(e.to_string()))
                    .map(|metadata| ExplicitTrackBranch {
                        display_path: metadata.display_path,
                        expected_branch: metadata.branch,
                        status: metadata.status,
                    })
            })
            .transpose()?;

        // Fail-closed: non-track-branch commits are always rejected.
        match repo
            .current_branch()
            .map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?
            .as_deref()
        {
            Some(branch) if branch.starts_with("track/") => {}
            Some("HEAD") => {
                return Err(GitWorkflowError::Unavailable(
                    "detached HEAD: switch to a track branch before committing".to_owned(),
                ));
            }
            Some(_) => {
                return Err(GitWorkflowError::Unavailable(
                    "non-track branch: switch to a track branch before committing".to_owned(),
                ));
            }
            None => {
                return Err(GitWorkflowError::Unavailable(
                    "cannot determine current git branch; switch to a track branch before committing"
                        .to_owned(),
                ));
            }
        }

        if let Some(explicit_track) = explicit_track.as_ref() {
            let current =
                repo.current_branch().map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?;
            verify_explicit_track_branch(current.as_deref(), explicit_track)
                .map_err(|e| GitWorkflowError::Unavailable(format!("Branch guard: {e}")))?;
        } else {
            let current =
                repo.current_branch().map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?;
            let claims = collect_track_branch_claims(repo.root())
                .map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?
                .into_iter()
                .map(|claim| TrackBranchClaim {
                    track_name: claim.track_name,
                    branch: claim.branch,
                    status: claim.status,
                })
                .collect::<Vec<_>>();
            verify_auto_detected_branch(current.as_deref(), &claims)
                .map_err(|e| GitWorkflowError::Unavailable(format!("Branch guard: {e}")))?;
        }

        let path_str = resolved.to_string_lossy().into_owned();
        let code = repo
            .status(&["commit", "-F", path_str.as_str()])
            .map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?;
        if code == 0 {
            if cleanup {
                let _ = fs::remove_file(&resolved);
            }
            Ok(())
        } else {
            Err(GitWorkflowError::Unavailable(format!("git commit failed with exit code {code}")))
        }
    }

    fn note_from_file(&self, path: &Path, cleanup: bool) -> GitWorkflowResult<()> {
        let repo =
            SystemGitRepo::discover().map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?;
        let resolved = resolve_repo_file_path(repo.root(), path, "git note file")?;
        ensure_existing_nonempty_file(&resolved, "git note file")?;
        let path_str = resolved.to_string_lossy().into_owned();
        let code = repo
            .status(&["notes", "add", "-f", "-F", path_str.as_str(), "HEAD"])
            .map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?;
        if code == 0 {
            if cleanup {
                let _ = fs::remove_file(&resolved);
            }
            Ok(())
        } else {
            Err(GitWorkflowError::Unavailable(format!(
                "git notes add failed with exit code {code}"
            )))
        }
    }

    fn switch_and_pull(&self, branch: &str) -> GitWorkflowResult<String> {
        let repo =
            SystemGitRepo::discover().map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?;
        let mut stdout_lines = Vec::<String>::new();

        stdout_lines.push(format!("Switching to {branch}..."));
        match repo
            .status(&["checkout", branch])
            .map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?
        {
            0 => {}
            code => {
                return Ok(format!("Failed to checkout {branch} (exit {code})"));
            }
        }

        stdout_lines.push(format!("Pulling latest from origin/{branch}..."));
        match repo
            .status(&["pull", "--ff-only"])
            .map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?
        {
            0 => {
                stdout_lines.push(format!("[OK] On {branch}, up to date."));
            }
            _ => {
                stdout_lines
                    .push("[WARN] Pull failed (may not have remote tracking branch)".to_owned());
            }
        }
        Ok(stdout_lines.join("\n"))
    }

    fn unstage(&self, paths: &[PathBuf]) -> GitWorkflowResult<()> {
        let repo =
            SystemGitRepo::discover().map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?;
        let mut args = vec!["restore", "--staged", "--"];
        let path_strs: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        args.extend(path_strs.iter().map(String::as_str));
        let code = repo.status(&args).map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?;
        if code == 0 {
            Ok(())
        } else {
            Err(GitWorkflowError::Unavailable(format!(
                "git restore --staged failed with exit code {code}"
            )))
        }
    }

    fn current_branch_track_id(&self) -> GitWorkflowResult<Option<String>> {
        let branch = match SystemGitRepo::discover()
            .and_then(|r| r.current_branch())
            .map_err(|e| GitWorkflowError::Unavailable(e.to_string()))?
        {
            Some(b) => b,
            None => return Ok(None),
        };
        match track_resolution::resolve_track_id_from_branch(Some(&branch)) {
            Ok(id) => Ok(Some(id)),
            Err(track_resolution::TrackResolutionError::InvalidTrackId(slug, _)) => {
                Err(GitWorkflowError::Unavailable(format!(
                    "current branch 'track/{slug}' has an invalid track id; \
                     rename the branch or switch to a valid track branch before committing"
                )))
            }
            Err(_) => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn resolve_repo_file_path(root: &Path, path: &Path, label: &str) -> GitWorkflowResult<PathBuf> {
    ensure_trusted_root(root)?;
    if path.as_os_str().is_empty() {
        return Err(GitWorkflowError::Unavailable(format!("{label} path must not be empty")));
    }
    if path.is_absolute() {
        return Err(GitWorkflowError::Unavailable(format!(
            "{label} path must be repo-relative: {}",
            path.display()
        )));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(GitWorkflowError::Unavailable(format!(
            "{label} path cannot escape the repo root: {}",
            path.display()
        )));
    }

    let resolved = root.join(path);
    if !resolved.starts_with(root) {
        return Err(GitWorkflowError::Unavailable(format!(
            "{label} path must stay within repo root: {}",
            path.display()
        )));
    }
    crate::track::symlink_guard::reject_symlinks_below(&resolved, root).map_err(|err| {
        GitWorkflowError::Unavailable(format!(
            "failed to validate {label} {}: {err}",
            resolved.display()
        ))
    })?;
    Ok(resolved)
}

fn ensure_trusted_root(root: &Path) -> GitWorkflowResult<()> {
    match root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => Err(GitWorkflowError::Unavailable(format!(
            "refusing to use symlinked repository root: {}",
            root.display()
        ))),
        Ok(_) => Ok(()),
        Err(err) => Err(GitWorkflowError::Unavailable(format!(
            "failed to stat repository root {}: {err}",
            root.display()
        ))),
    }
}

fn ensure_existing_nonempty_file(path: &Path, label: &str) -> GitWorkflowResult<()> {
    if !path.is_file() {
        return Err(GitWorkflowError::Unavailable(format!("Missing {label}: {}", path.display())));
    }
    let content = fs::read_to_string(path).map_err(|err| {
        GitWorkflowError::Unavailable(format!("failed to read {label} {}: {err}", path.display()))
    })?;
    if content.trim().is_empty() {
        return Err(GitWorkflowError::Unavailable(format!("{label} is empty: {}", path.display())));
    }
    Ok(())
}

fn load_stage_paths(path: &Path) -> GitWorkflowResult<Vec<String>> {
    ensure_existing_nonempty_file(path, "stage path list file")?;
    let content = fs::read_to_string(path).map_err(|err| {
        GitWorkflowError::Unavailable(format!(
            "failed to read stage path list {}: {err}",
            path.display()
        ))
    })?;
    validate_stage_path_entries(content.lines()).map_err(|err| {
        let msg = err.to_string();
        if msg == "Stage path list file has no usable entries" {
            GitWorkflowError::Unavailable(format!("{msg}: {}", path.display()))
        } else {
            err
        }
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::Path;

    use super::resolve_repo_file_path;

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
}
