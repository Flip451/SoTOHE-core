//! `ReviewService` — single aggregate facade consumed by `ReviewDriver`.
//!
//! Collapses the 11 individual service handles that `ReviewDriver` previously
//! held into one injected interactor, satisfying the D3/D4 cli_driver policy:
//! "Driver translates input → invokes one injected interactor → renders".
//!
//! The concrete implementation lives in `cli_composition` and wires all
//! individual sub-services internally.

use std::path::PathBuf;

use crate::commit_hash_persistence::CommitHashPersistenceError;
use crate::review_v2::{
    ReviewApprovalOutput, ReviewCheckApprovedError, ReviewRunLocalOutput, RunReviewError,
    RunReviewFixError, RunReviewFixOutput, RunReviewOutput,
};

// ── Input DTOs ────────────────────────────────────────────────────────────────

/// Input for the `RunCodex` / `RunClaude` review variants.
#[allow(clippy::exhaustive_structs)]
pub struct ReviewRunInput {
    pub model: String,
    pub timeout_seconds: u64,
    pub briefing_file: Option<PathBuf>,
    pub prompt: Option<String>,
    pub track_id: Option<String>,
    pub round_type: String,
    pub group: String,
    pub items_dir: PathBuf,
}

/// Input for the `RunFixLocal` review variant.
#[allow(clippy::exhaustive_structs)]
pub struct ReviewRunFixInput {
    pub scope: String,
    pub briefing_file: PathBuf,
    pub track_id: String,
    pub round_type: String,
    pub model: Option<String>,
}

// ── ReviewService trait ───────────────────────────────────────────────────────

/// Aggregate primary port for the `review` command family.
///
/// `ReviewDriver` holds exactly one `Arc<dyn ReviewService>` and delegates
/// each `ReviewInput` variant to the corresponding method.  The concrete
/// implementation (`ReviewServiceImpl` in `cli_composition`) wires all
/// individual sub-services internally, keeping the wiring complexity out of
/// the driver.
pub trait ReviewService: Send + Sync {
    /// Run the Codex-backed reviewer.
    fn run_codex(&self, input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError>;

    /// Run the Claude-backed reviewer.
    fn run_claude(&self, input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError>;

    /// Run the provider-auto-resolved local reviewer.
    #[allow(clippy::too_many_arguments)]
    fn run_local(
        &self,
        model: Option<String>,
        timeout_seconds: u64,
        briefing_file: Option<PathBuf>,
        prompt: Option<String>,
        track_id: Option<String>,
        round_type: String,
        group: String,
        items_dir: PathBuf,
    ) -> ReviewRunLocalOutput;

    /// Run the review-fix-lead fixer.
    fn run_fix_local(
        &self,
        input: ReviewRunFixInput,
    ) -> Result<RunReviewFixOutput, RunReviewFixError>;

    /// Check if review is approved and code hash is current.
    fn check_approved(
        &self,
        track_id: String,
        items_dir: PathBuf,
    ) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError>;

    /// Render review results output.
    #[allow(clippy::too_many_arguments)]
    fn results(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
        scope: Option<String>,
        all: bool,
        limit: u32,
        round_type: String,
        no_hint: bool,
    ) -> Result<String, String>;

    /// Classify each path string into its review scope(s).
    fn classify(
        &self,
        paths: Vec<String>,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<(String, String)>, String>;

    /// List the diff files belonging to the given scope.
    fn files(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<String>, String>;

    /// Validate a scope name for the given track.
    fn validate_scope(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<(), String>;

    /// Get the briefing file path for the given scope.
    fn get_briefing(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Option<String>, String>;

    /// Persist the HEAD SHA to `.commit_hash` for the given track.
    fn persist_commit_hash(
        &self,
        track_id: String,
        workspace_root: PathBuf,
    ) -> Result<String, CommitHashPersistenceError>;
}
