//! `review_v2` command family — CliApp impl methods.

mod inputs;

pub use inputs::{
    ReviewResultsInput, ReviewRunClaudeInput, ReviewRunCodexInput, ReviewRunLocalInput,
};

use std::path::PathBuf;

use crate::{CliApp, CommandOutcome};

impl CliApp {
    /// Run the local Codex-backed reviewer and auto-record verdict to review.json.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn review_run_codex(&self, input: ReviewRunCodexInput) -> Result<CommandOutcome, String> {
        let _ = input;
        Err(String::from("not implemented"))
    }

    /// Run the local Claude-backed reviewer and auto-record verdict to review.json.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn review_run_claude(&self, input: ReviewRunClaudeInput) -> Result<CommandOutcome, String> {
        let _ = input;
        Err(String::from("not implemented"))
    }

    /// Run the local reviewer with provider auto-resolved from agent-profiles.json.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn review_run_local(&self, input: ReviewRunLocalInput) -> Result<CommandOutcome, String> {
        let _ = input;
        Err(String::from("not implemented"))
    }

    /// Check if the review state is approved and code hash is current.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn review_check_approved(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, String> {
        let _ = (track_id, items_dir);
        Err(String::from("not implemented"))
    }

    /// Show review results: per-scope state summary, optional round history.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn review_results(&self, input: ReviewResultsInput) -> Result<CommandOutcome, String> {
        let _ = input;
        Err(String::from("not implemented"))
    }

    /// Classify each given path into review scopes.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn review_classify(
        &self,
        paths: Vec<String>,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, String> {
        let _ = (paths, track_id, items_dir);
        Err(String::from("not implemented"))
    }

    /// List the diff files belonging to the given scope.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn review_files(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, String> {
        let _ = (scope, track_id, items_dir);
        Err(String::from("not implemented"))
    }

    /// Validate a scope name for the given track.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn review_validate_scope(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, String> {
        let _ = (scope, track_id, items_dir);
        Err(String::from("not implemented"))
    }

    /// Get the briefing for a review scope.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn review_get_briefing(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, String> {
        let _ = (scope, track_id, items_dir);
        Err(String::from("not implemented"))
    }

    /// Persist a commit hash for the review cycle.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn review_persist_commit_hash(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, String> {
        let _ = (track_id, items_dir);
        Err(String::from("not implemented"))
    }
}
