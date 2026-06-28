//! `ReviewCompositionRoot` definition and `ReviewServiceImpl` — the concrete
//! implementation of `usecase::review_v2::ReviewService`.
//!
//! `ReviewServiceImpl` wires all 11 individual sub-services internally so that
//! `ReviewDriver` holds only one `Arc<dyn ReviewService>` (D3/D4 cli_driver
//! policy).

use std::path::PathBuf;
use std::sync::Arc;

use usecase::commit_hash_persistence::CommitHashPersistenceError;
use usecase::review_v2::aggregate_service::{ReviewRunFixInput, ReviewRunInput, ReviewService};
use usecase::review_v2::review_aux::ReviewAuxError;
use usecase::review_v2::{
    ReviewApprovalOutput, ReviewCheckApprovedError, ReviewRunLocalOutput, RunReviewError,
    RunReviewFixError, RunReviewFixOutput, RunReviewOutput,
};

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

    /// Construct a fully-wired [`cli_driver::review::ReviewDriver`].
    ///
    /// Wires `ReviewServiceImpl` (which holds all 11 sub-services internally)
    /// and injects it as a single `Arc<dyn ReviewService>` into the driver
    /// (D3/D4 cli_driver policy).
    pub fn review_driver(&self) -> cli_driver::review::ReviewDriver {
        let service = Arc::new(ReviewServiceImpl) as Arc<dyn ReviewService>;
        cli_driver::review::ReviewDriver::new(service)
    }
}

impl Default for ReviewCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

// ── ReviewServiceImpl ─────────────────────────────────────────────────────────

/// Concrete implementation of [`ReviewService`] that delegates to the
/// `ReviewCompositionRoot` methods.
///
/// All wiring complexity stays here; `ReviewDriver` holds only one
/// `Arc<dyn ReviewService>`.
pub(crate) struct ReviewServiceImpl;

impl ReviewService for ReviewServiceImpl {
    fn run_codex(&self, input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError> {
        let root = ReviewCompositionRoot::new();
        let comp_input = super::ReviewRunCodexInput {
            model: input.model,
            timeout_seconds: input.timeout_seconds,
            briefing_file: input.briefing_file,
            prompt: input.prompt,
            track_id: input.track_id,
            round_type: input.round_type,
            group: input.group,
            items_dir: input.items_dir,
        };
        match root.review_run_codex(comp_input) {
            Ok(outcome) => Ok(RunReviewOutput {
                verdict_kind: if outcome.exit_code == 0 {
                    "approved".to_owned()
                } else {
                    "rejected".to_owned()
                },
                skipped: false,
                finding_count: 0,
                summary: outcome.stdout,
                exit_code: outcome.exit_code,
            }),
            Err(e) => Err(RunReviewError::ReviewerFailed(e.to_string())),
        }
    }

    fn run_claude(&self, input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError> {
        let root = ReviewCompositionRoot::new();
        let comp_input = super::ReviewRunClaudeInput {
            model: input.model,
            timeout_seconds: input.timeout_seconds,
            briefing_file: input.briefing_file,
            prompt: input.prompt,
            track_id: input.track_id,
            round_type: input.round_type,
            group: input.group,
            items_dir: input.items_dir,
        };
        match root.review_run_claude(comp_input) {
            Ok(outcome) => Ok(RunReviewOutput {
                verdict_kind: if outcome.exit_code == 0 {
                    "approved".to_owned()
                } else {
                    "rejected".to_owned()
                },
                skipped: false,
                finding_count: 0,
                summary: outcome.stdout,
                exit_code: outcome.exit_code,
            }),
            Err(e) => Err(RunReviewError::ReviewerFailed(e.to_string())),
        }
    }

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
    ) -> ReviewRunLocalOutput {
        let root = ReviewCompositionRoot::new();
        let input = super::ReviewRunLocalInput {
            model,
            timeout_seconds,
            briefing_file,
            prompt,
            track_id,
            round_type,
            group,
            items_dir,
        };
        match root.review_run_local(input) {
            Ok(outcome) => ReviewRunLocalOutput {
                stdout: outcome.stdout,
                stderr: outcome.stderr,
                exit_code: outcome.exit_code,
            },
            Err(e) => {
                ReviewRunLocalOutput { stdout: None, stderr: Some(e.to_string()), exit_code: 1 }
            }
        }
    }

    fn run_fix_local(
        &self,
        input: ReviewRunFixInput,
    ) -> Result<RunReviewFixOutput, RunReviewFixError> {
        let root = ReviewCompositionRoot::new();
        let comp_input = super::RunReviewFixLocalInput {
            scope: input.scope,
            briefing_file: input.briefing_file,
            track_id: input.track_id,
            round_type: input.round_type,
            model: input.model,
        };
        match root.review_run_fix_local(comp_input) {
            Ok(outcome) => {
                // Detect claude subagent dispatch sentinel: exit 64 +
                // `SUBAGENT_DISPATCH_REQUIRED` on the first stdout line.
                // The composition root already handles the claude provider
                // path correctly — this shim must NOT map exit 64 to
                // "failed" and overwrite stdout with "REVIEW_FIX_STATUS:
                // failed". Propagate as a typed error so the driver can
                // pass the payload through verbatim without knowing the
                // sentinel string or exit code itself.
                let stdout_str = outcome.stdout.as_deref().unwrap_or("");
                if outcome.exit_code == super::run_fix::SUBAGENT_DISPATCH_EXIT_CODE
                    && stdout_str.starts_with(super::run_fix::SUBAGENT_DISPATCH_SENTINEL)
                {
                    let payload = outcome.stdout.unwrap_or_default();
                    return Err(RunReviewFixError::SubagentDispatchRequired(payload));
                }
                // Smoke-test failures from cli_composition::review_v2::run_fix
                // return exit 2 with `[ERROR] smoke test failed: ...` on stderr
                // and no stdout sentinel. Distinguish them from genuine
                // `REVIEW_FIX_STATUS: blocked_cross_scope` (which has the
                // sentinel on stdout) by inspecting whether the runner emitted
                // a sentinel — exit 2 alone is ambiguous between "blocked" and
                // "smoke test failed". Propagate the smoke-test case as a
                // typed `SmokeTestFailed` error so the driver can preserve
                // exit 2 + stderr diagnostic without inventing a fake status
                // string.
                let stdout_has_sentinel =
                    outcome.stdout.as_deref().is_some_and(|s| s.contains("REVIEW_FIX_STATUS:"));
                if outcome.exit_code == 2 && !stdout_has_sentinel {
                    let msg = outcome
                        .stderr
                        .unwrap_or_else(|| "smoke test failed (no diagnostic)".to_owned());
                    return Err(RunReviewFixError::SmokeTestFailed(msg));
                }
                let exit_code = i32::from(outcome.exit_code);
                let status = match outcome.exit_code {
                    0 => "completed",
                    2 => "blocked_cross_scope",
                    _ => "failed",
                }
                .to_owned();
                Ok(RunReviewFixOutput { status, exit_code, stderr: outcome.stderr })
            }
            Err(e) => Err(RunReviewFixError::FixRunnerFailed(e.to_string())),
        }
    }

    fn check_approved(
        &self,
        track_id: String,
        items_dir: PathBuf,
    ) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError> {
        super::approved::check_approved_str(&track_id, &items_dir)
    }

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
    ) -> Result<String, ReviewAuxError> {
        // `all` selects every scope, NOT every history entry. The history
        // selector is owned by the `ResultsLimit::All` parsing path in cli
        // (which substitutes `u32::MAX` when the user passes `--limit all`).
        // Forwarding `limit` unchanged here preserves the default `--limit 0`
        // summary semantics when only `--all` is set.
        let root = ReviewCompositionRoot::new();
        let input = super::ReviewResultsInput {
            track_id,
            items_dir,
            scope,
            all,
            limit,
            round_type,
            no_hint,
        };
        root.review_results(input)
            .map(|o| o.stdout.unwrap_or_default())
            .map_err(|e| ReviewAuxError::Failed(e.to_string()))
    }

    fn classify(
        &self,
        paths: Vec<String>,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<(String, String)>, ReviewAuxError> {
        let root = ReviewCompositionRoot::new();
        root.review_classify(paths, track_id, items_dir)
            .map(|outcome| {
                let stdout = outcome.stdout.unwrap_or_default();
                stdout
                    .lines()
                    .filter_map(|line| {
                        let mut parts = line.splitn(2, '\t');
                        let path = parts.next()?.to_owned();
                        let scopes = parts.next().unwrap_or("").to_owned();
                        Some((path, scopes))
                    })
                    .collect()
            })
            .map_err(|e| ReviewAuxError::Failed(e.to_string()))
    }

    fn files(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<String>, ReviewAuxError> {
        let root = ReviewCompositionRoot::new();
        root.review_files(scope, track_id, items_dir)
            .map(|outcome| {
                let stdout = outcome.stdout.unwrap_or_default();
                stdout.lines().map(str::to_owned).collect()
            })
            .map_err(|e| ReviewAuxError::Failed(e.to_string()))
    }

    fn validate_scope(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<(), ReviewAuxError> {
        let root = ReviewCompositionRoot::new();
        root.review_validate_scope(scope, track_id, items_dir)
            .map(|_| ())
            .map_err(|e| ReviewAuxError::Failed(e.to_string()))
    }

    fn get_briefing(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Option<String>, ReviewAuxError> {
        let root = ReviewCompositionRoot::new();
        root.review_get_briefing(scope, track_id, items_dir)
            .map(|outcome| outcome.stdout)
            .map_err(|e| ReviewAuxError::Failed(e.to_string()))
    }

    fn persist_commit_hash(
        &self,
        track_id: String,
        _workspace_root: PathBuf,
    ) -> Result<String, CommitHashPersistenceError> {
        super::commit_hash::persist_commit_hash_for_track(&track_id)
            .map_err(CommitHashPersistenceError::StoreWriteFailed)
    }
}
