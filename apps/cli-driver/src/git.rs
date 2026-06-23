//! `git` command family — primary adapter driver.
//!
//! `GitDriver` holds an injected [`usecase::git_workflow::GitWorkflowService`] and
//! exposes `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::git_workflow::GitWorkflowService;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `git` command family.
pub enum GitInput {
    /// Stage the whole worktree except transient automation scratch files.
    AddAll,
    /// Stage repo-relative paths listed in a file.
    AddFromFile {
        /// Path to the file containing repo-relative paths to stage (one per line).
        path: PathBuf,
        /// Remove the paths file after staging.
        cleanup: bool,
    },
    /// Create a commit using the message stored in a file.
    CommitFromFile {
        /// Path to the file containing the commit message.
        path: PathBuf,
        /// Remove the commit message file after committing.
        cleanup: bool,
        /// Optional track directory for branch guard validation.
        track_dir: Option<PathBuf>,
    },
    /// Attach a git note using the contents of a file.
    NoteFromFile {
        /// Path to the file containing the note body.
        path: PathBuf,
        /// Remove the note file after attaching.
        cleanup: bool,
    },
    /// Switch to a branch and pull latest changes.
    SwitchAndPull {
        /// Branch name to check out and pull.
        branch: String,
    },
    /// Unstage paths (remove from git index without discarding worktree changes).
    Unstage {
        /// Paths to remove from the index.
        paths: Vec<PathBuf>,
    },
    /// Resolve the track ID from the current git branch (strict mode).
    CurrentBranchTrackIdStrict,
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `git` command family.
///
/// Holds an injected [`GitWorkflowService`]; exposes `handle(input) -> CommandOutcome`.
pub struct GitDriver {
    service: Arc<dyn GitWorkflowService>,
}

impl GitDriver {
    /// Create a new `GitDriver` with the given git workflow service.
    pub fn new(service: Arc<dyn GitWorkflowService>) -> Self {
        Self { service }
    }

    /// Handle a git command.
    pub fn handle(&self, input: GitInput) -> CommandOutcome {
        match input {
            GitInput::AddAll => self.git_add_all(),
            GitInput::AddFromFile { path, cleanup } => self.git_add_from_file(path, cleanup),
            GitInput::CommitFromFile { path, cleanup, track_dir } => {
                self.git_commit_from_file(path, cleanup, track_dir)
            }
            GitInput::NoteFromFile { path, cleanup } => self.git_note_from_file(path, cleanup),
            GitInput::SwitchAndPull { branch } => self.git_switch_and_pull(branch),
            GitInput::Unstage { paths } => self.git_unstage(paths),
            GitInput::CurrentBranchTrackIdStrict => self.current_branch_track_id_strict_outcome(),
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn git_add_all(&self) -> CommandOutcome {
        match self.service.stage_all() {
            Ok(()) => CommandOutcome::success(None),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn git_add_from_file(&self, path: PathBuf, cleanup: bool) -> CommandOutcome {
        match self.service.stage_from_file(&path, cleanup) {
            Ok(()) => CommandOutcome::success(None),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn git_commit_from_file(
        &self,
        path: PathBuf,
        cleanup: bool,
        track_dir: Option<PathBuf>,
    ) -> CommandOutcome {
        match self.service.commit_from_file(&path, cleanup, track_dir.as_deref()) {
            Ok(()) => CommandOutcome::success(None),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn git_note_from_file(&self, path: PathBuf, cleanup: bool) -> CommandOutcome {
        match self.service.note_from_file(&path, cleanup) {
            Ok(()) => CommandOutcome::success(None),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn git_switch_and_pull(&self, branch: String) -> CommandOutcome {
        match self.service.switch_and_pull(&branch) {
            Ok(msg) => CommandOutcome::success(Some(msg)),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn git_unstage(&self, paths: Vec<PathBuf>) -> CommandOutcome {
        match self.service.unstage(&paths) {
            Ok(()) => CommandOutcome::success(None),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn current_branch_track_id_strict_outcome(&self) -> CommandOutcome {
        match self.service.current_branch_track_id() {
            Ok(Some(id)) => CommandOutcome::success(Some(id)),
            Ok(None) => CommandOutcome::success(None),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }
}
