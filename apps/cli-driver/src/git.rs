//! `git` command family — primary adapter driver.
//!
//! `GitDriver` holds an injected [`usecase::git_workflow::GitWorkflowService`] and
//! exposes `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::git_stash::{GitStashCommand, GitStashService};
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
    /// Fast-forward pull the current branch (`git pull --ff-only`).
    Sync,
    /// Unstage paths (remove from git index without discarding worktree changes).
    Unstage {
        /// Paths to remove from the index.
        paths: Vec<PathBuf>,
    },
    /// Resolve the track ID from the current git branch (strict mode).
    CurrentBranchTrackIdStrict,
}

/// Typed primary-adapter input for guarded stash operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitStashInput {
    /// Save tracked and untracked worktree changes.
    Push,
    /// Restore the most recent saved worktree.
    Pop,
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `git` command family.
///
/// Holds an injected [`GitWorkflowService`]; exposes `handle(input) -> CommandOutcome`.
pub struct GitDriver {
    service: Arc<dyn GitWorkflowService>,
    stash_service: Arc<dyn GitStashService>,
}

impl GitDriver {
    /// Create a new `GitDriver` with the git workflow and stash services.
    pub fn new(
        service: Arc<dyn GitWorkflowService>,
        stash_service: Arc<dyn GitStashService>,
    ) -> Self {
        Self { service, stash_service }
    }

    /// Handle a guarded stash command.
    pub fn handle_stash(&self, input: GitStashInput) -> CommandOutcome {
        let command = match input {
            GitStashInput::Push => GitStashCommand::Push,
            GitStashInput::Pop => GitStashCommand::Pop,
        };
        match self.stash_service.execute(command) {
            Ok(()) => CommandOutcome::success(None),
            Err(error) => CommandOutcome::failure(Some(error.to_string())),
        }
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
            GitInput::Sync => self.git_sync(),
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

    fn git_sync(&self) -> CommandOutcome {
        match self.service.sync_current_branch() {
            Ok(()) => CommandOutcome::success(None),
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
            Ok(Some(id)) => CommandOutcome::success(Some(id.as_ref().to_owned())),
            Ok(None) => CommandOutcome::success(None),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use usecase::git_stash::{GitStashCommand, GitStashError, GitStashService};
    use usecase::git_workflow::{DiagnosticText, GitWorkflowError, GitWorkflowService};

    use super::{GitDriver, GitStashInput};

    struct UnusedWorkflowService;

    impl GitWorkflowService for UnusedWorkflowService {
        fn stage_all(&self) -> Result<(), GitWorkflowError> {
            unused_workflow_call()
        }

        fn stage_from_file(&self, _path: &Path, _cleanup: bool) -> Result<(), GitWorkflowError> {
            unused_workflow_call()
        }

        fn commit_from_file(
            &self,
            _path: &Path,
            _cleanup: bool,
            _track_dir: Option<&Path>,
        ) -> Result<(), GitWorkflowError> {
            unused_workflow_call()
        }

        fn note_from_file(&self, _path: &Path, _cleanup: bool) -> Result<(), GitWorkflowError> {
            unused_workflow_call()
        }

        fn unstage(&self, _paths: &[PathBuf]) -> Result<(), GitWorkflowError> {
            unused_workflow_call()
        }

        fn current_branch_track_id(&self) -> Result<Option<domain::TrackId>, GitWorkflowError> {
            unused_workflow_call()
        }

        fn sync_current_branch(&self) -> Result<(), GitWorkflowError> {
            unused_workflow_call()
        }
    }

    fn unused_workflow_call<T>() -> Result<T, GitWorkflowError> {
        Err(GitWorkflowError::Message(DiagnosticText::new(
            "workflow service must not be called by stash handling",
        )))
    }

    struct RecordingStashService {
        commands: Mutex<Vec<GitStashCommand>>,
        fail: bool,
    }

    impl RecordingStashService {
        fn succeeding() -> Self {
            Self { commands: Mutex::new(Vec::new()), fail: false }
        }

        fn failing() -> Self {
            Self { commands: Mutex::new(Vec::new()), fail: true }
        }
    }

    impl GitStashService for RecordingStashService {
        fn execute(&self, command: GitStashCommand) -> Result<(), GitStashError> {
            self.commands.lock().unwrap().push(command);
            if self.fail { Err(GitStashError::ForbiddenBranchRefUpdate) } else { Ok(()) }
        }
    }

    fn driver(stash_service: Arc<dyn GitStashService>) -> GitDriver {
        GitDriver::new(Arc::new(UnusedWorkflowService), stash_service)
    }

    #[test]
    fn test_handle_stash_push_and_pop_translate_and_render_success() {
        let stash_service = Arc::new(RecordingStashService::succeeding());
        let driver = driver(stash_service.clone());

        let push = driver.handle_stash(GitStashInput::Push);
        let pop = driver.handle_stash(GitStashInput::Pop);

        assert_eq!(push.stdout, None);
        assert_eq!(push.stderr, None);
        assert_eq!(push.exit_code, 0);
        assert_eq!(pop.stdout, None);
        assert_eq!(pop.stderr, None);
        assert_eq!(pop.exit_code, 0);
        assert_eq!(
            *stash_service.commands.lock().unwrap(),
            vec![GitStashCommand::Push, GitStashCommand::Pop]
        );
    }

    #[test]
    fn test_handle_stash_service_error_renders_failure_exit_status() {
        let stash_service = Arc::new(RecordingStashService::failing());
        let outcome = driver(stash_service.clone()).handle_stash(GitStashInput::Push);

        assert_eq!(outcome.stdout, None);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("guarded stash attempted a forbidden branch-ref update")
        );
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(*stash_service.commands.lock().unwrap(), vec![GitStashCommand::Push]);
    }
}
