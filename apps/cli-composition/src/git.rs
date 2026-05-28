//! `git` command family — CliApp impl methods.

use std::path::PathBuf;

use crate::{CliApp, CommandOutcome};

impl CliApp {
    /// Stage the whole worktree except transient automation scratch files.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn git_add_all(&self) -> Result<CommandOutcome, String> {
        Err(String::from("not implemented"))
    }

    /// Stage repo-relative paths listed in a file.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn git_add_from_file(
        &self,
        path: PathBuf,
        cleanup: bool,
    ) -> Result<CommandOutcome, String> {
        let _ = (path, cleanup);
        Err(String::from("not implemented"))
    }

    /// Create a commit using the message stored in a file.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn git_commit_from_file(
        &self,
        path: PathBuf,
        cleanup: bool,
        track_dir: Option<PathBuf>,
    ) -> Result<CommandOutcome, String> {
        let _ = (path, cleanup, track_dir);
        Err(String::from("not implemented"))
    }

    /// Attach a git note using the contents of a file.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn git_note_from_file(
        &self,
        path: PathBuf,
        cleanup: bool,
    ) -> Result<CommandOutcome, String> {
        let _ = (path, cleanup);
        Err(String::from("not implemented"))
    }

    /// Switch to a branch and pull latest changes.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn git_switch_and_pull(&self, branch: String) -> Result<CommandOutcome, String> {
        let _ = branch;
        Err(String::from("not implemented"))
    }

    /// Unstage paths (remove from git index without discarding worktree changes).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn git_unstage(&self, paths: Vec<PathBuf>) -> Result<CommandOutcome, String> {
        let _ = paths;
        Err(String::from("not implemented"))
    }
}
