//! `track` command family — core CliApp impl methods.

mod tddd;

use std::path::PathBuf;

use crate::{CliApp, CommandOutcome};

impl CliApp {
    /// Transition a task to a new status.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_transition(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        task_id: String,
        target_status: String,
        commit_hash: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id, task_id, target_status, commit_hash);
        Err(String::from("not implemented"))
    }

    /// Create a new track branch from main.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_branch_create(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id);
        Err(String::from("not implemented"))
    }

    /// Switch to an existing track branch.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_branch_switch(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id);
        Err(String::from("not implemented"))
    }

    /// Resolve the current track phase, next command, and blocker.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_resolve(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id);
        Err(String::from("not implemented"))
    }

    /// Validate metadata.json files under the repository.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_views_validate(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }

    /// Render plan.md and registry.md from metadata.json.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_views_sync(
        &self,
        project_root: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (project_root, track_id);
        Err(String::from("not implemented"))
    }

    /// Add a new task to a track.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_add_task(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        description: String,
        section: Option<String>,
        after: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id, description, section, after);
        Err(String::from("not implemented"))
    }

    /// Set a status override on a track (blocked/cancelled).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_set_override(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        status: String,
        reason: String,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id, status, reason);
        Err(String::from("not implemented"))
    }

    /// Clear a status override on a track.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_clear_override(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id);
        Err(String::from("not implemented"))
    }

    /// Show the next open task for a track (JSON output).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_next_task(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id);
        Err(String::from("not implemented"))
    }

    /// Show task status counts for a track (JSON output).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_task_counts(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id);
        Err(String::from("not implemented"))
    }

    /// Evaluate spec.md source tags and store results in metadata.json spec_signals.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_signals(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id);
        Err(String::from("not implemented"))
    }
}
