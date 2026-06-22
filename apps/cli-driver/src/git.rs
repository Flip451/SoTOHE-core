// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `git` command family — primary adapter driver.
//!
//! `GitDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The logic here mirrors
//! `apps/cli-composition/src/git.rs`; T021 removes the `cli_composition`
//! duplicate when the live path is flipped.

// TODO(T021): add use-case + infrastructure imports once Cargo.toml is materialized.
// use std::fs;
// use std::path::{Path, PathBuf};
// use std::sync::Arc;
// use infrastructure::git_cli::GitRepository as _;
// use infrastructure::git_cli::{SystemGitRepo, collect_track_branch_claims, load_explicit_track_branch};
// use usecase::git_workflow::{TRANSIENT_AUTOMATION_DIRS, TRANSIENT_AUTOMATION_FILES, ...};

use std::path::PathBuf;

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
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct GitDriver {
    // TODO(T021): inject use-case interactors here.
    // git_workflow_service: Arc<dyn usecase::git_workflow::GitWorkflowService>,
}

impl GitDriver {
    /// Create a new `GitDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a git command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
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
    // Render helpers (logic duplicated from cli_composition/src/git.rs;
    // T021 removes the cli_composition copy).
    // -----------------------------------------------------------------------

    fn git_add_all(&self) -> CommandOutcome {
        // TODO(T021): invoke GitWorkflowInteractor here.
        // Mirrors cli_composition/src/git.rs GitCompositionRoot::git_add_all.
        CommandOutcome::success(None)
    }

    fn git_add_from_file(&self, _path: PathBuf, _cleanup: bool) -> CommandOutcome {
        // TODO(T021): invoke GitWorkflowInteractor here.
        // Mirrors cli_composition/src/git.rs GitCompositionRoot::git_add_from_file.
        CommandOutcome::success(None)
    }

    fn git_commit_from_file(
        &self,
        _path: PathBuf,
        _cleanup: bool,
        _track_dir: Option<PathBuf>,
    ) -> CommandOutcome {
        // TODO(T021): invoke GitWorkflowInteractor here.
        // Mirrors cli_composition/src/git.rs GitCompositionRoot::git_commit_from_file.
        CommandOutcome::success(None)
    }

    fn git_note_from_file(&self, _path: PathBuf, _cleanup: bool) -> CommandOutcome {
        // TODO(T021): invoke GitWorkflowInteractor here.
        // Mirrors cli_composition/src/git.rs GitCompositionRoot::git_note_from_file
        // (lines ~179-207).
        CommandOutcome::success(None)
    }

    fn git_switch_and_pull(&self, branch: String) -> CommandOutcome {
        // TODO(T021): invoke GitWorkflowInteractor here.
        // Mirrors cli_composition/src/git.rs GitCompositionRoot::git_switch_and_pull.
        let stdout = format!("Switching to {branch}...\nPulling latest from origin/{branch}...\n[OK] On {branch}, up to date.");
        CommandOutcome::success(Some(stdout))
    }

    fn git_unstage(&self, _paths: Vec<PathBuf>) -> CommandOutcome {
        // TODO(T021): invoke GitWorkflowInteractor here.
        // Mirrors cli_composition/src/git.rs GitCompositionRoot::git_unstage.
        CommandOutcome::success(None)
    }

    fn current_branch_track_id_strict_outcome(&self) -> CommandOutcome {
        // TODO(T021): invoke use-case here.
        // Mirrors cli_composition/src/git.rs GitCompositionRoot::current_branch_track_id_strict.
        CommandOutcome::success(None)
    }
}

impl Default for GitDriver {
    fn default() -> Self {
        Self::new()
    }
}
