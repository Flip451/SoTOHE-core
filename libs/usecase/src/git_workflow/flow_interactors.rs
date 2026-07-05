//! Track / PR / review_v2 git-flow interactors (D8 orchestration relocation).
//!
//! Private submodule of [`crate::git_workflow`]; every interactor is
//! re-exported from the parent so the public rustdoc path stays
//! `usecase::git_workflow::*`.

use std::path::Path;
use std::sync::Arc;

use super::{DiagnosticText, GitPrimitivePort, GitWorkflowError, TrackArchiveFsPort};

/// UseCase interactor for the git-involved track subcommands
/// (`create_track_branch` / `switch_to_track_branch` / `switch_to_base` /
/// `archive_track`), relocated from apps/cli-composition/src/track/.
///
/// Holds injected [`GitPrimitivePort`] + [`TrackArchiveFsPort`] adapters so the
/// orchestration is testable via mock ports and does not depend on
/// infrastructure directly.
///
/// IN-05 / IN-08 / IN-10 / CN-03 / CN-05 / AC-04 / AC-09.
pub struct TrackGitInteractor {
    git: Arc<dyn GitPrimitivePort>,
    fs: Arc<dyn TrackArchiveFsPort>,
}

impl TrackGitInteractor {
    /// Inject the git-primitive and archive-fs ports.
    #[must_use]
    pub fn new(git: Arc<dyn GitPrimitivePort>, fs: Arc<dyn TrackArchiveFsPort>) -> Self {
        Self { git, fs }
    }

    /// Create a new track branch off `base_branch` and check it out.
    ///
    /// Verifies the caller is currently on `base_branch` and that
    /// `track/<track_id>` does not already exist before creating.
    ///
    /// # Errors
    /// Returns `GitWorkflowError::Message` when the caller is not on
    /// `base_branch` or when the target branch already exists, and propagates
    /// port errors from the underlying git primitives.
    pub fn create_track_branch(
        &self,
        project_root: &Path,
        track_id: &domain::TrackId,
        base_branch: &str,
    ) -> Result<(), GitWorkflowError> {
        let branch_name = format!("track/{track_id}");
        let current = self.git.current_branch(Some(project_root))?;
        if current.as_deref() != Some(base_branch) {
            return Err(GitWorkflowError::Message(DiagnosticText::new(format!(
                "branch create must start from '{base_branch}'; current branch is {}",
                current.as_deref().unwrap_or("<detached>")
            ))));
        }
        if self.git.branch_exists(Some(project_root), &branch_name)? {
            return Err(GitWorkflowError::Message(DiagnosticText::new(format!(
                "branch '{branch_name}' already exists"
            ))));
        }
        self.git.create_branch(Some(project_root), &branch_name, base_branch)
    }

    /// Switch to an existing `track/<track_id>` branch.
    ///
    /// Verifies the branch exists before switching.
    ///
    /// # Errors
    /// Returns `GitWorkflowError::Message` when the branch does not exist,
    /// and propagates port errors from the underlying git primitives.
    pub fn switch_to_track_branch(
        &self,
        project_root: &Path,
        track_id: &domain::TrackId,
    ) -> Result<String, GitWorkflowError> {
        let branch_name = format!("track/{track_id}");
        if !self.git.branch_exists(Some(project_root), &branch_name)? {
            return Err(GitWorkflowError::Message(DiagnosticText::new(format!(
                "branch '{branch_name}' does not exist"
            ))));
        }
        self.git.switch_branch(Some(project_root), &branch_name)?;
        Ok(format!("[OK] Switched to branch: {branch_name}"))
    }

    /// Switch to the base branch and sync (ff-only) — the D6 flow that
    /// replaces the legacy `switch-and-pull` combined command.
    ///
    /// # Errors
    /// Propagates port errors from `switch_branch` and unexpected
    /// `sync_current_branch` failures. Known sync refusal modes are folded into
    /// the `[WARN] Pull failed` non-fatal message that pins CN-05 behavior.
    pub fn switch_to_base(
        &self,
        project_root: &Path,
        base_branch: &str,
    ) -> Result<String, GitWorkflowError> {
        let mut lines = Vec::<String>::new();
        lines.push(format!("Switching to {base_branch}..."));
        self.git.switch_branch(Some(project_root), base_branch)?;
        lines.push(format!("Pulling latest from origin/{base_branch}..."));
        match self.git.sync_current_branch(Some(project_root)) {
            Ok(()) => {
                lines.push(format!("[OK] On {base_branch}, up to date."));
            }
            Err(
                GitWorkflowError::SyncUpstreamNotSet
                | GitWorkflowError::SyncNonFastForward { .. }
                | GitWorkflowError::SyncWorktreeUnresolved { .. }
                | GitWorkflowError::Unavailable(_),
            ) => {
                // CN-05 bit-equivalence: the previous inline
                // `git_switch_and_pull_impl` in cli_composition treated every
                // non-zero `git pull --ff-only` result as `[WARN] Pull failed`
                // and returned exit 0 with the switch success. Preserve that
                // observable stdout / exit contract by folding **all** sync
                // failures — the three classified fail-closed modes plus
                // Unavailable (from `SyncError::Spawn`: auth / network / process
                // spawn errors) — into the same WARN line. The base checkout
                // has already succeeded and the `/track:done` contract
                // ("sync attempted, not guaranteed") requires callers not to be
                // stranded by a transient pull failure.
                lines.push("[WARN] Pull failed (may not have remote tracking branch)".to_owned());
            }
            Err(e) => return Err(e),
        }
        Ok(lines.join("\n"))
    }

    /// Archive a completed track: `git mv track/items/<id> track/archive/<id>`
    /// and, if a gitignored `logs/` directory was present, `mv` it to the new
    /// location using the filesystem port.
    ///
    /// # Errors
    /// Propagates git-port and fs-port errors. Returns
    /// `GitWorkflowError::Message` when the source track directory is
    /// missing or the destination already exists.
    pub fn archive_track(
        &self,
        project_root: &Path,
        track_id: &domain::TrackId,
    ) -> Result<String, GitWorkflowError> {
        let items_dir = project_root.join("track").join("items");
        let src_dir = items_dir.join(track_id.as_ref());
        if !self.fs.path_is_dir(&src_dir)? {
            return Err(GitWorkflowError::Message(DiagnosticText::new(format!(
                "track directory not found: {}",
                src_dir.display()
            ))));
        }
        let archive_root = project_root.join("track").join("archive");
        let dst_dir = archive_root.join(track_id.as_ref());
        if self.fs.path_exists(&dst_dir)? {
            return Err(GitWorkflowError::Message(DiagnosticText::new(format!(
                "archive destination already exists: {}",
                dst_dir.display()
            ))));
        }
        self.fs.create_dir_all(&archive_root)?;

        let src_logs = src_dir.join("logs");
        let logs_was_dir = self.fs.path_is_dir(&src_logs)?;

        self.git.move_path(Some(project_root), &src_dir, &dst_dir)?;

        if logs_was_dir {
            let dst_logs = dst_dir.join("logs");
            if !self.fs.path_is_dir(&dst_logs)? {
                if !self.fs.path_is_dir(&src_logs)? {
                    let rollback_message =
                        describe_archive_rollback(self.rollback_archive_contents_after_logs_error(
                            project_root,
                            &src_dir,
                            &dst_dir,
                        ));
                    return Err(GitWorkflowError::Fs {
                        detail: DiagnosticText::new(format!(
                            "logs/ was present before archive but was not found at {} or {} after git mv; {rollback_message}",
                            src_logs.display(),
                            dst_logs.display()
                        )),
                    });
                }
                if let Err(rename_err) = self.fs.rename_path(&src_logs, &dst_logs) {
                    // Rollback the git mv so the track directory is not left
                    // partially moved. If rollback also fails, surface both
                    // failures so callers know the archive may be inconsistent.
                    let rollback_message =
                        describe_archive_rollback(self.rollback_archive_contents_after_logs_error(
                            project_root,
                            &src_dir,
                            &dst_dir,
                        ));
                    return Err(GitWorkflowError::Fs {
                        detail: DiagnosticText::new(format!(
                            "failed to move logs from {} to {}: {rename_err}; {rollback_message}",
                            src_logs.display(),
                            dst_logs.display()
                        )),
                    });
                }
            }
        }

        Ok(format!(
            "[OK] Archived track '{track_id}': {} → {}",
            src_dir.display(),
            dst_dir.display()
        ))
    }

    fn rollback_archive_contents_after_logs_error(
        &self,
        project_root: &Path,
        src_dir: &Path,
        dst_dir: &Path,
    ) -> Result<(), GitWorkflowError> {
        if !self.fs.path_exists(dst_dir)? {
            return Ok(());
        }

        self.fs.create_dir_all(src_dir)?;
        for dst_child in self.fs.list_dir_file_names(dst_dir)? {
            let file_name = dst_child.file_name().ok_or_else(|| GitWorkflowError::Fs {
                detail: DiagnosticText::new(format!(
                    "failed to roll back archive move from {}: missing file name",
                    dst_child.display()
                )),
            })?;
            let src_child = src_dir.join(file_name);
            self.git.move_path(Some(project_root), &dst_child, &src_child).map_err(|e| {
                GitWorkflowError::Fs {
                    detail: DiagnosticText::new(format!(
                        "failed to roll back archive move from {} to {}: {e}",
                        dst_child.display(),
                        src_child.display()
                    )),
                }
            })?;
        }
        self.fs.remove_dir(dst_dir)
    }
}

fn describe_archive_rollback(result: Result<(), GitWorkflowError>) -> String {
    match result {
        Ok(()) => "rollback succeeded".to_owned(),
        Err(err) => format!("rollback failed: {err}"),
    }
}

/// UseCase interactor for the pr subcommand's git reads
/// (`fetch_and_read_metadata_at_ref` / `resolve_head`), relocated from
/// apps/cli-composition/src/pr.rs + pr/poll.rs.
///
/// IN-08 / IN-10 / CN-03 / AC-09.
pub struct PrGitInteractor {
    git: Arc<dyn GitPrimitivePort>,
}

impl PrGitInteractor {
    /// Inject the git-primitive port.
    #[must_use]
    pub fn new(git: Arc<dyn GitPrimitivePort>) -> Self {
        Self { git }
    }

    /// Fetch `origin/<branch>` and return the `track/items/<id>/metadata.json`
    /// contents at that ref.
    ///
    /// # Errors
    /// Propagates port errors from `fetch_branch` / `show_file_at_ref`.
    pub fn fetch_and_read_metadata_at_ref(
        &self,
        branch: &str,
        track_id: &domain::TrackId,
    ) -> Result<String, GitWorkflowError> {
        self.git.fetch_branch(None, branch)?;
        let git_ref = format!("origin/{branch}");
        let metadata_path = Path::new("track/items").join(track_id.as_ref()).join("metadata.json");
        self.git.show_file_at_ref(None, &git_ref, &metadata_path)
    }

    /// Resolve HEAD to a [`domain::CommitHash`].
    ///
    /// # Errors
    /// Propagates port errors from `resolve_commit`.
    pub fn resolve_head(&self) -> Result<Option<domain::CommitHash>, GitWorkflowError> {
        self.git.resolve_commit(None, "HEAD")
    }
}

/// UseCase interactor for the review_v2 subcommand's git reads
/// (`resolve_head_for_track_branch` / `resolve_diff_base`), relocated from
/// apps/cli-composition/src/review_v2/commit_hash.rs + shared.rs.
///
/// IN-08 / IN-10 / CN-03 / AC-09.
pub struct ReviewGitInteractor {
    git: Arc<dyn GitPrimitivePort>,
}

impl ReviewGitInteractor {
    /// Inject the git-primitive port.
    #[must_use]
    pub fn new(git: Arc<dyn GitPrimitivePort>) -> Self {
        Self { git }
    }

    /// Resolve HEAD to a [`domain::CommitHash`] for a track branch.
    ///
    /// # Errors
    /// Returns `GitWorkflowError::Message` when HEAD cannot be resolved
    /// (e.g. an empty repo). Propagates other port errors.
    pub fn resolve_head_for_track_branch(
        &self,
        track_id: &domain::TrackId,
    ) -> Result<domain::CommitHash, GitWorkflowError> {
        match self.git.resolve_commit(None, "HEAD")? {
            Some(hash) => Ok(hash),
            None => Err(GitWorkflowError::Message(DiagnosticText::new(format!(
                "cannot resolve HEAD for track '{track_id}'"
            )))),
        }
    }

    /// Resolve an arbitrary rev to a [`domain::CommitHash`]. Used by the
    /// review-diff base resolver, so callers that supply an invalid rev
    /// receive `Ok(None)` rather than an error.
    ///
    /// # Errors
    /// Propagates port errors from `resolve_commit`.
    pub fn resolve_diff_base(
        &self,
        rev: &str,
    ) -> Result<Option<domain::CommitHash>, GitWorkflowError> {
        self.git.resolve_commit(None, rev)
    }
}
