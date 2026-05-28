//! `make` command family — CliApp impl methods.

use crate::{CliApp, CommandOutcome};

impl CliApp {
    /// Run CI then commit with the given message.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_commit(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Attach a git note to HEAD.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_note(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Run CI then commit using tmp/track-commit/commit-message.txt.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_commit_message(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Create a track branch from main.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_branch_create(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Switch to an existing track branch.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_branch_switch(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Resolve current track phase.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_resolve(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Push current track/plan branch to origin.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_pr_push(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Create or reuse a PR for the current branch.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_pr_ensure(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Push + ensure PR in one step.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_pr(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Run full PR review cycle.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_pr_review(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Wait for PR checks then merge.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_pr_merge(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Show PR check status.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_pr_status(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Run the local Codex planner.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_local_plan(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Run the local Codex reviewer.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_local_review(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Show per-scope review results (state summary by default).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_review_results(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Check that the review state is approved and code hash is current.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_check_approved(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Switch to main branch and pull latest.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_switch_main(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Stage paths from tmp/track-commit/add-paths.txt.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_add_paths(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Transition a task status.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_transition(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Add a new task to a track.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_add_task(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Show the next open task (JSON).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_next_task(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Show task status counts (JSON).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_task_counts(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Set or clear a status override.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_set_override(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Render plan.md and registry.md from metadata.json.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_sync_views(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Attach git note from tmp/track-commit/note.md.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_note(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Write current HEAD SHA to .commit_hash (set v2 diff base).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_track_set_commit_hash(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Stage all worktree changes.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_add_all(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Unstage paths (remove from index without discarding worktree changes).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_unstage(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }

    /// Run a cargo make task via tools-daemon exec with WORKER_ID isolation.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn make_exec(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let _ = raw_args;
        Err(String::from("not implemented"))
    }
}
