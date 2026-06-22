//! `ReviewCompositionRoot` definition and `impl CliApp` delegation shims for
//! the `review_v2` command family.
//!
//! Each `CliApp` method forwards to `ReviewCompositionRoot::new().method(...)`,
//! preserving `apps/cli` call sites unchanged during the per-context dissolution
//! migration (T013).

use std::path::PathBuf;

use super::inputs::{
    ReviewResultsInput, ReviewRunClaudeInput, ReviewRunCodexInput, ReviewRunLocalInput,
    RunReviewFixLocalInput,
};
use crate::{CliApp, CommandOutcome, error::CompositionError};

// ── Per-context composition root ──────────────────────────────────────────────

/// Composition root for the `review_v2` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct ReviewCompositionRoot;

impl ReviewCompositionRoot {
    /// Create a new `ReviewCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReviewCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

// ── CliApp delegation shims ───────────────────────────────────────────────────

impl CliApp {
    /// Delegates to [`ReviewCompositionRoot::review_run_codex`].
    pub fn review_run_codex(
        &self,
        input: ReviewRunCodexInput,
    ) -> Result<CommandOutcome, CompositionError> {
        ReviewCompositionRoot::new().review_run_codex(input)
    }

    /// Delegates to [`ReviewCompositionRoot::review_run_claude`].
    pub fn review_run_claude(
        &self,
        input: ReviewRunClaudeInput,
    ) -> Result<CommandOutcome, CompositionError> {
        ReviewCompositionRoot::new().review_run_claude(input)
    }

    /// Delegates to [`ReviewCompositionRoot::review_run_local`].
    pub fn review_run_local(
        &self,
        input: ReviewRunLocalInput,
    ) -> Result<CommandOutcome, CompositionError> {
        ReviewCompositionRoot::new().review_run_local(input)
    }

    /// Delegates to [`ReviewCompositionRoot::review_run_fix_local`].
    pub fn review_run_fix_local(
        &self,
        input: RunReviewFixLocalInput,
    ) -> Result<CommandOutcome, CompositionError> {
        ReviewCompositionRoot::new().review_run_fix_local(input)
    }

    /// Delegates to [`ReviewCompositionRoot::review_check_approved`].
    pub fn review_check_approved(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        ReviewCompositionRoot::new().review_check_approved(track_id, items_dir)
    }

    /// Delegates to [`ReviewCompositionRoot::review_results`].
    pub fn review_results(
        &self,
        input: ReviewResultsInput,
    ) -> Result<CommandOutcome, CompositionError> {
        ReviewCompositionRoot::new().review_results(input)
    }

    /// Delegates to [`ReviewCompositionRoot::review_classify`].
    pub fn review_classify(
        &self,
        paths: Vec<String>,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        ReviewCompositionRoot::new().review_classify(paths, track_id, items_dir)
    }

    /// Delegates to [`ReviewCompositionRoot::review_files`].
    pub fn review_files(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        ReviewCompositionRoot::new().review_files(scope, track_id, items_dir)
    }

    /// Delegates to [`ReviewCompositionRoot::review_validate_scope`].
    pub fn review_validate_scope(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        ReviewCompositionRoot::new().review_validate_scope(scope, track_id, items_dir)
    }

    /// Delegates to [`ReviewCompositionRoot::review_get_briefing`].
    pub fn review_get_briefing(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        ReviewCompositionRoot::new().review_get_briefing(scope, track_id, items_dir)
    }

    /// Delegates to [`ReviewCompositionRoot::review_persist_commit_hash`].
    pub fn review_persist_commit_hash(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        ReviewCompositionRoot::new().review_persist_commit_hash(track_id, items_dir)
    }
}
