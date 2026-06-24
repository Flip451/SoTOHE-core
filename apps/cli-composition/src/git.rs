//! `git` command family — per-context composition root and CliApp shim.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use infrastructure::git_cli::GitRepository as _;

use crate::{CommandOutcome, error::CompositionError};

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `git` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct GitCompositionRoot;

impl GitCompositionRoot {
    /// Create a new `GitCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl GitCompositionRoot {
    /// Stage the whole worktree except transient automation scratch files.
    ///
    /// # Errors
    /// Returns `Err` when git discovery or staging fails.
    pub fn git_add_all(&self) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::git_cli::SystemGitRepo;
        use usecase::git_workflow::TRANSIENT_AUTOMATION_DIRS;
        use usecase::git_workflow::TRANSIENT_AUTOMATION_FILES;

        let repo =
            SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
        repo.stage_all_excluding(TRANSIENT_AUTOMATION_FILES, TRANSIENT_AUTOMATION_DIRS)
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        Ok(CommandOutcome::success(None))
    }

    /// Stage repo-relative paths listed in a file.
    ///
    /// # Errors
    /// Returns `Err` when git discovery, file reading, or staging fails.
    pub fn git_add_from_file(
        &self,
        path: PathBuf,
        cleanup: bool,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::git_cli::SystemGitRepo;

        let repo =
            SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
        let path = repo.resolve_path(&path);

        let stage_paths = load_stage_paths(&path)?;

        let mut owned_args = vec!["add".to_owned(), "--".to_owned()];
        owned_args.extend(stage_paths);
        let args: Vec<&str> = owned_args.iter().map(String::as_str).collect();
        let code =
            repo.status(&args).map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        if code == 0 {
            if cleanup {
                let _ = fs::remove_file(&path);
            }
            Ok(CommandOutcome::success(None))
        } else {
            Ok(CommandOutcome::failure(None))
        }
    }

    /// Create a commit using the message stored in a file.
    ///
    /// # Errors
    /// Returns `Err` when git discovery, branch guard, or commit fails.
    pub fn git_commit_from_file(
        &self,
        path: PathBuf,
        cleanup: bool,
        track_dir: Option<PathBuf>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::git_cli::{
            SystemGitRepo, collect_track_branch_claims, load_explicit_track_branch,
        };
        use usecase::git_workflow::{
            ExplicitTrackBranch, TrackBranchClaim, verify_auto_detected_branch,
            verify_explicit_track_branch,
        };

        let repo =
            SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
        let path = repo.resolve_path(&path);

        ensure_existing_nonempty_file(&path, "commit message file")?;

        let explicit_track = track_dir
            .map(|td| {
                let resolved_td = repo.resolve_path(&td);
                load_explicit_track_branch(repo.root(), &resolved_td)
                    .map(|metadata| ExplicitTrackBranch {
                        display_path: metadata.display_path,
                        expected_branch: metadata.branch,
                        status: metadata.status,
                    })
                    .map_err(|e| CompositionError::WiringFailed(e.to_string()))
            })
            .transpose()?;

        // Fail-closed: non-track-branch commits are always rejected.
        match repo
            .current_branch()
            .map_err(|e| CompositionError::AdapterInit(e.to_string()))?
            .as_deref()
        {
            Some(branch) if branch.starts_with("track/") => {}
            Some("HEAD") => {
                return Err(CompositionError::WiringFailed(
                    "detached HEAD: switch to a track branch before committing".to_owned(),
                ));
            }
            Some(_) => {
                return Err(CompositionError::WiringFailed(
                    "non-track branch: switch to a track branch before committing".to_owned(),
                ));
            }
            None => {
                return Err(CompositionError::WiringFailed(
                    "cannot determine current git branch; switch to a track branch before committing"
                        .to_owned(),
                ));
            }
        }

        if let Some(explicit_track) = explicit_track.as_ref() {
            let current =
                repo.current_branch().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
            verify_explicit_track_branch(current.as_deref(), explicit_track)
                .map_err(|e| CompositionError::WiringFailed(format!("Branch guard: {e}")))?;
        } else {
            let current =
                repo.current_branch().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
            let claims = collect_track_branch_claims(repo.root())
                .map_err(|e| CompositionError::Infrastructure(e.to_string()))?
                .into_iter()
                .map(|claim| TrackBranchClaim {
                    track_name: claim.track_name,
                    branch: claim.branch,
                    status: claim.status,
                })
                .collect::<Vec<_>>();
            verify_auto_detected_branch(current.as_deref(), &claims)
                .map_err(|e| CompositionError::WiringFailed(format!("Branch guard: {e}")))?;
        }

        let path_str = path.to_string_lossy().into_owned();
        let code = repo
            .status(&["commit", "-F", path_str.as_str()])
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        if code == 0 {
            if cleanup {
                let _ = fs::remove_file(&path);
            }
            Ok(CommandOutcome::success(None))
        } else {
            Ok(CommandOutcome::failure(None))
        }
    }

    /// Attach a git note using the contents of a file.
    ///
    /// # Errors
    /// Returns `Err` when git discovery, file reading, or note attachment fails.
    pub fn git_note_from_file(
        &self,
        path: PathBuf,
        cleanup: bool,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::git_cli::SystemGitRepo;

        let repo =
            SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
        let path = repo.resolve_path(&path);
        ensure_existing_nonempty_file(&path, "git note file")?;
        let path_str = path.to_string_lossy().into_owned();
        let code = repo
            .status(&["notes", "add", "-f", "-F", path_str.as_str(), "HEAD"])
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        if code == 0 {
            if cleanup {
                let _ = fs::remove_file(&path);
            }
            Ok(CommandOutcome::success(None))
        } else {
            Ok(CommandOutcome::failure(None))
        }
    }

    /// Switch to a branch and pull latest changes.
    ///
    /// # Errors
    /// Returns `Err` when git discovery or checkout fails.
    pub fn git_switch_and_pull(&self, branch: String) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::git_cli::SystemGitRepo;

        let repo =
            SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
        let mut stdout_lines = Vec::<String>::new();

        stdout_lines.push(format!("Switching to {branch}..."));
        match repo
            .status(&["checkout", &branch])
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?
        {
            0 => {}
            code => {
                return Ok(CommandOutcome {
                    stdout: Some(format!("Failed to checkout {branch}")),
                    stderr: None,
                    exit_code: u8::try_from(code).unwrap_or(1),
                });
            }
        }

        stdout_lines.push(format!("Pulling latest from origin/{branch}..."));
        match repo
            .status(&["pull", "--ff-only"])
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?
        {
            0 => {
                stdout_lines.push(format!("[OK] On {branch}, up to date."));
            }
            _ => {
                stdout_lines
                    .push("[WARN] Pull failed (may not have remote tracking branch)".to_owned());
            }
        }
        Ok(CommandOutcome::success(Some(stdout_lines.join("\n"))))
    }

    /// Unstage paths (remove from git index without discarding worktree changes).
    ///
    /// # Errors
    /// Returns `Err` when git discovery or unstage fails.
    pub fn git_unstage(&self, paths: Vec<PathBuf>) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::git_cli::SystemGitRepo;

        let repo =
            SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
        let mut args = vec!["restore", "--staged", "--"];
        let path_strs: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        args.extend(path_strs.iter().map(String::as_str));
        let code =
            repo.status(&args).map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        if code == 0 {
            Ok(CommandOutcome::success(None))
        } else {
            Ok(CommandOutcome {
                stdout: None,
                stderr: None,
                exit_code: u8::try_from(code).unwrap_or(1),
            })
        }
    }

    /// Resolve the track ID from the current git branch (strict mode).
    ///
    /// Returns `Ok(Some(id))` only when the branch matches `track/<id>` and the id is valid.
    /// Non-track branches (e.g. `main`) return `Ok(None)`.
    /// Returns `Err` when the branch matches `track/<id>` but the `<id>` fails validation.
    ///
    /// # Errors
    /// Returns a typed composition error when validation of the track ID fails.
    pub fn current_branch_track_id_strict(&self) -> Result<Option<String>, CompositionError> {
        use infrastructure::git_cli::GitRepository as _;
        use infrastructure::git_cli::SystemGitRepo;

        let branch = match SystemGitRepo::discover().and_then(|r| r.current_branch()) {
            Ok(Some(b)) => b,
            Ok(None) | Err(_) => return Ok(None),
        };
        match usecase::track_resolution::resolve_track_id_from_branch(Some(&branch)) {
            Ok(id) => Ok(Some(id)),
            Err(usecase::track_resolution::TrackResolutionError::InvalidTrackId(slug, _)) => {
                Err(CompositionError::Usecase(format!(
                    "current branch 'track/{slug}' has an invalid track id; \
                     rename the branch or switch to a valid track branch before committing"
                )))
            }
            Err(_) => Ok(None),
        }
    }

    /// Build a wired [`cli_driver::git::GitDriver`] for the git family.
    pub fn git_driver(&self) -> cli_driver::git::GitDriver {
        use infrastructure::FsGitWorkflowAdapter;
        use usecase::git_workflow::GitWorkflowInteractor;

        let port = Arc::new(FsGitWorkflowAdapter::new());
        let service = Arc::new(GitWorkflowInteractor::new(port));
        cli_driver::git::GitDriver::new(service)
    }
}

fn ensure_existing_nonempty_file(path: &Path, label: &str) -> Result<(), CompositionError> {
    if !path.is_file() {
        return Err(CompositionError::WiringFailed(format!("Missing {label}: {}", path.display())));
    }
    let content = fs::read_to_string(path).map_err(|err| {
        CompositionError::WiringFailed(format!("failed to read {label} {}: {err}", path.display()))
    })?;
    if content.trim().is_empty() {
        return Err(CompositionError::WiringFailed(format!(
            "{label} is empty: {}",
            path.display()
        )));
    }
    Ok(())
}

fn load_stage_paths(path: &Path) -> Result<Vec<String>, CompositionError> {
    ensure_existing_nonempty_file(path, "stage path list file")?;

    let content = fs::read_to_string(path).map_err(|err| {
        CompositionError::Infrastructure(format!(
            "failed to read stage path list {}: {err}",
            path.display()
        ))
    })?;
    use usecase::git_workflow::validate_stage_path_entries;
    validate_stage_path_entries(content.lines()).map_err(|err| {
        let msg = err.to_string();
        if msg == "Stage path list file has no usable entries" {
            CompositionError::WiringFailed(format!("{msg}: {}", path.display()))
        } else {
            CompositionError::WiringFailed(msg)
        }
    })
}

// Restored from baseline 883cb682 (apps/cli/src/commands/git.rs).
// These `load_stage_paths` tests were dropped during the cli-composition
// migration. The behavior (dedup + transient-automation rejection via the
// usecase `validate_stage_path_entries` rules) is unchanged, so the coverage
// is restored here.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;

    use super::load_stage_paths;

    #[test]
    fn load_stage_paths_accepts_unique_repo_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        let list = dir.path().join("add-paths.txt");
        fs::write(&list, "src/lib.rs\n# comment\nsrc/lib.rs\nREADME.md\n").unwrap();

        let paths = load_stage_paths(&list).unwrap();

        assert_eq!(paths, vec!["src/lib.rs".to_owned(), "README.md".to_owned()]);
    }

    #[test]
    fn load_stage_paths_rejects_transient_automation_directory() {
        let dir = tempfile::tempdir().unwrap();
        let list = dir.path().join("add-paths.txt");
        fs::write(&list, "tmp/track-commit\n").unwrap();

        let err = load_stage_paths(&list).unwrap_err();

        assert!(err.to_string().contains("transient automation"));
    }
}
