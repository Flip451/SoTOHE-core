//! `review_v2` command family — composition logic and CliApp impl methods.

pub mod approved;
pub mod briefing;
mod inputs;
pub mod null_reviewer;
pub mod results;
pub mod run;
pub mod scope;
pub mod shared;

pub use inputs::{
    ReviewResultsInput, ReviewRunClaudeInput, ReviewRunCodexInput, ReviewRunLocalInput,
};

// Public re-exports: all composition types and free functions used by callers
// of the cli_composition crate (e.g. apps/cli shim, future callers).
pub use approved::{build_check_approved_service, check_approved_str};
pub use briefing::{append_scope_briefing_reference_str, get_briefing_for_scope_str};
pub use null_reviewer::NullReviewer;
pub use results::{build_run_review_service, render_review_results_str};
pub use run::{run_claude_review_str, run_codex_review_str};
pub use scope::{
    load_scope_config_only, load_scope_config_only_str, validate_review_group_name_str,
    validate_scope_for_track_str, validate_track_id_str,
};
pub use shared::{
    CodexReviewOutcome, NullDiffGetter, ReviewV2Composition, ReviewV2CompositionWithClaude,
    ReviewV2CompositionWithCodex, build_review_v2, build_review_v2_str,
    build_review_v2_with_claude_reviewer, build_review_v2_with_claude_reviewer_str,
    build_review_v2_with_reviewer, build_review_v2_with_reviewer_str,
    build_scope_query_interactor_no_diff_str, build_scope_query_interactor_str,
    resolve_diff_base_and_getter,
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
