//! `review` command family — primary adapter driver.
//!
//! `ReviewDriver` holds a single injected `ReviewService` aggregate and exposes
//! `handle(input) -> CommandOutcome`. Each operation delegates to the
//! appropriate method on the service without importing `infrastructure` or
//! `domain`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::review_v2::SubagentDispatchInstruction;
use usecase::review_v2::aggregate_service::ReviewRunInput;
use usecase::review_v2::run_review_fix::{
    ReviewFixRunnerError, RunReviewFixError, RunReviewFixOutput, RunReviewFixRequest,
    RunReviewFixService,
};
use usecase::review_v2::{
    ReviewCheckZeroFindingsEvaluationError, ReviewCheckZeroFindingsOutcome,
    ReviewCheckZeroFindingsQuery, ReviewCheckZeroFindingsValidationError,
};
use usecase::review_v2::{
    ReviewCheckZeroFindingsService, ReviewResultsService, ReviewScopeSelectionRequest,
    ReviewScopeSelectionValidationError, ReviewService,
};

use crate::render::CommandOutcome;

#[path = "review_results_renderer.rs"]
mod review_results_renderer;
use review_results_renderer::render_review_results;
#[path = "review_local_output_renderer.rs"]
mod review_local_output_renderer;
use review_local_output_renderer::review_run_local_output_to_outcome;

/// First stdout line for a review-fix subagent dispatch.
pub const SUBAGENT_DISPATCH_SENTINEL: &str = "SUBAGENT_DISPATCH_REQUIRED";

/// Exit code for a review-fix subagent dispatch.
pub const SUBAGENT_DISPATCH_EXIT_CODE: u8 = 64;

/// Converts the check-zero-findings evaluation into the command's fail-closed
/// exit-code contract.
fn check_zero_findings_outcome_to_command_outcome(
    outcome: ReviewCheckZeroFindingsOutcome,
) -> CommandOutcome {
    match outcome {
        ReviewCheckZeroFindingsOutcome::CurrentFinalZeroFindings => CommandOutcome::success(Some(
            "current final review verdict is zero_findings".to_owned(),
        )),
        ReviewCheckZeroFindingsOutcome::MissingFinalVerdict => CommandOutcome::failure(Some(
            "no final review verdict exists for this scope".to_owned(),
        )),
        ReviewCheckZeroFindingsOutcome::StaleFinalVerdict => {
            CommandOutcome::failure(Some("final review verdict is stale for this scope".to_owned()))
        }
        ReviewCheckZeroFindingsOutcome::FindingsRemain => {
            CommandOutcome::failure(Some("final review verdict has findings remaining".to_owned()))
        }
    }
}

/// Maps both a completed check and an evaluation error to the command boundary.
fn check_zero_findings_result_to_command_outcome(
    result: Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsEvaluationError>,
) -> CommandOutcome {
    match result {
        Ok(outcome) => check_zero_findings_outcome_to_command_outcome(outcome),
        Err(error) => CommandOutcome::failure(Some(error.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Final-only round selector accepted by the check-zero-findings delivery boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewCheckRoundSelect {
    Final,
}

/// Delivery input that converts validated CLI values into the focused usecase query.
#[derive(Debug, Clone)]
pub struct ReviewCheckZeroFindingsInput(ReviewCheckZeroFindingsQuery);

impl ReviewCheckZeroFindingsInput {
    /// Forwards raw delivery values to the usecase-owned query constructor.
    pub fn try_new(
        items_dir: PathBuf,
        track_id: String,
        scope: String,
        round: ReviewCheckRoundSelect,
    ) -> Result<Self, ReviewCheckZeroFindingsValidationError> {
        match round {
            ReviewCheckRoundSelect::Final => {
                ReviewCheckZeroFindingsQuery::try_new(items_dir, track_id, scope).map(Self)
            }
        }
    }

    /// Returns the usecase-validated query for dispatch.
    #[must_use]
    pub fn into_query(self) -> ReviewCheckZeroFindingsQuery {
        self.0
    }
}

/// Delivery input for `review results` that owns raw CLI values until the
/// usecase selection constructor validates them.
#[derive(Debug, Clone)]
pub struct ReviewResultsInput {
    track_id: Option<String>,
    items_dir: PathBuf,
    request: ReviewScopeSelectionRequest,
    limit: u32,
    round_type: String,
    no_hint: bool,
}

impl ReviewResultsInput {
    /// Converts raw delivery values into a format-valid usecase request.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        track_id: Option<String>,
        items_dir: PathBuf,
        scope: Option<String>,
        all: bool,
        limit: u32,
        round_type: String,
        no_hint: bool,
    ) -> Result<Self, ReviewScopeSelectionValidationError> {
        let request = ReviewScopeSelectionRequest::try_new(scope, all)?;
        Ok(Self { track_id, items_dir, request, limit, round_type, no_hint })
    }

    /// Decomposes the input for a single aggregate-service call.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (Option<String>, PathBuf, ReviewScopeSelectionRequest, u32, String, bool) {
        (self.track_id, self.items_dir, self.request, self.limit, self.round_type, self.no_hint)
    }
}

/// Typed input for the `review` command family.
pub enum ReviewInput {
    /// Run the local Codex-backed reviewer and auto-record verdict to review.json.
    RunCodex(String, u64, Option<PathBuf>, Option<String>, Option<String>, String, String, PathBuf),
    /// Run the Claude-backed reviewer and auto-record verdict to review.json.
    RunClaude(
        String,
        u64,
        Option<PathBuf>,
        Option<String>,
        Option<String>,
        String,
        String,
        PathBuf,
    ),
    /// Run the auto-dispatched local reviewer (provider resolved from agent-profiles.json).
    RunLocal(
        Option<String>,
        u64,
        Option<PathBuf>,
        Option<String>,
        Option<String>,
        String,
        String,
        PathBuf,
    ),
    /// Check if the review state is approved and code hash is current.
    CheckApproved(String, PathBuf),
    /// Check whether one resolved track and scope have a current final
    /// zero-findings review verdict.
    CheckZeroFindings(ReviewCheckZeroFindingsInput),
    /// Show review results: per-scope state summary, optional round history.
    Results(ReviewResultsInput),
    /// Classify each given path into review scopes.
    Classify(Vec<String>, Option<String>, PathBuf),
    /// List the diff files belonging to the given scope.
    Files(String, Option<String>, PathBuf),
    /// Validate a scope name for the given track.
    ValidateScope(String, Option<String>, PathBuf),
    /// Get the briefing for a review scope.
    GetBriefing(String, Option<String>, PathBuf),
    /// Persist a commit hash for the review cycle.
    PersistCommitHash(String, PathBuf),
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `review` command family.
///
/// Holds separately injected aggregate and check-zero-findings services;
/// exposes `handle(input) -> CommandOutcome`.
pub struct ReviewDriver {
    service: Arc<dyn ReviewService>,
    results: Arc<dyn ReviewResultsService>,
    check_zero_findings: Arc<dyn ReviewCheckZeroFindingsService>,
}

impl ReviewDriver {
    /// Create a new `ReviewDriver` with its aggregate and focused services.
    pub fn new(
        service: Arc<dyn ReviewService>,
        results: Arc<dyn ReviewResultsService>,
        check_zero_findings: Arc<dyn ReviewCheckZeroFindingsService>,
    ) -> Self {
        Self { service, results, check_zero_findings }
    }

    /// Handle a review command.
    pub fn handle(&self, input: ReviewInput) -> CommandOutcome {
        match input {
            ReviewInput::RunCodex(
                model,
                timeout_seconds,
                briefing_file,
                prompt,
                track_id,
                round_type,
                group,
                items_dir,
            ) => self.review_run_codex(
                model,
                timeout_seconds,
                briefing_file,
                prompt,
                track_id,
                round_type,
                group,
                items_dir,
            ),
            ReviewInput::RunClaude(
                model,
                timeout_seconds,
                briefing_file,
                prompt,
                track_id,
                round_type,
                group,
                items_dir,
            ) => self.review_run_claude(
                model,
                timeout_seconds,
                briefing_file,
                prompt,
                track_id,
                round_type,
                group,
                items_dir,
            ),
            ReviewInput::RunLocal(
                model,
                timeout_seconds,
                briefing_file,
                prompt,
                track_id,
                round_type,
                group,
                items_dir,
            ) => self.review_run_local(
                model,
                timeout_seconds,
                briefing_file,
                prompt,
                track_id,
                round_type,
                group,
                items_dir,
            ),
            ReviewInput::CheckApproved(track_id, items_dir) => {
                self.review_check_approved(track_id, items_dir)
            }
            ReviewInput::CheckZeroFindings(input) => {
                self.review_check_zero_findings(input.into_query())
            }
            ReviewInput::Results(input) => self.review_results(input),
            ReviewInput::Classify(paths, track_id, items_dir) => {
                self.review_classify(paths, track_id, items_dir)
            }
            ReviewInput::Files(scope, track_id, items_dir) => {
                self.review_files(scope, track_id, items_dir)
            }
            ReviewInput::ValidateScope(scope, track_id, items_dir) => {
                self.review_validate_scope(scope, track_id, items_dir)
            }
            ReviewInput::GetBriefing(scope, track_id, items_dir) => {
                self.review_get_briefing(scope, track_id, items_dir)
            }
            ReviewInput::PersistCommitHash(track_id, workspace_root) => {
                self.review_persist_commit_hash(track_id, workspace_root)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Operation implementations
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn review_run_codex(
        &self,
        model: String,
        timeout_seconds: u64,
        briefing_file: Option<PathBuf>,
        prompt: Option<String>,
        track_id: Option<String>,
        round_type: String,
        group: String,
        items_dir: PathBuf,
    ) -> CommandOutcome {
        // Pass briefing_file and prompt through to the service; prompt resolution
        // (briefing_file → "Read <path> and perform..." expansion) is the
        // usecase layer's responsibility.
        let input = ReviewRunInput {
            model,
            timeout_seconds,
            briefing_file,
            prompt,
            track_id,
            round_type,
            group,
            items_dir,
        };
        match self.service.run_codex(input) {
            Ok(out) => run_review_output_to_outcome(out),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn review_run_claude(
        &self,
        model: String,
        timeout_seconds: u64,
        briefing_file: Option<PathBuf>,
        prompt: Option<String>,
        track_id: Option<String>,
        round_type: String,
        group: String,
        items_dir: PathBuf,
    ) -> CommandOutcome {
        // Pass briefing_file and prompt through to the service; prompt resolution
        // is the usecase layer's responsibility (mirrors review_run_codex).
        let input = ReviewRunInput {
            model,
            timeout_seconds,
            briefing_file,
            prompt,
            track_id,
            round_type,
            group,
            items_dir,
        };
        match self.service.run_claude(input) {
            Ok(out) => run_review_output_to_outcome(out),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn review_run_local(
        &self,
        model: Option<String>,
        timeout_seconds: u64,
        briefing_file: Option<PathBuf>,
        prompt: Option<String>,
        track_id: Option<String>,
        round_type: String,
        group: String,
        items_dir: PathBuf,
    ) -> CommandOutcome {
        let out = self.service.run_local(
            model,
            timeout_seconds,
            briefing_file,
            prompt,
            track_id,
            round_type,
            group,
            items_dir,
        );
        review_run_local_output_to_outcome(out)
    }

    fn review_check_approved(&self, track_id: String, items_dir: PathBuf) -> CommandOutcome {
        use usecase::review_v2::ReviewApprovalDecision;

        match self.service.check_approved(track_id, items_dir) {
            Ok(output) => {
                let (msg, exit_code) = match output.decision {
                    ReviewApprovalDecision::Approved => {
                        ("[OK] Review is approved and code hash is current".to_owned(), 0u8)
                    }
                    ReviewApprovalDecision::ApprovedWithBypass => {
                        let count = output.bypass_scope_count.unwrap_or(0);
                        (
                            format!(
                                "[WARN] No review.json found. Allowing commit for PR-based review \
                                 ({count} scope(s))."
                            ),
                            0u8,
                        )
                    }
                    ReviewApprovalDecision::Blocked => {
                        let mut display: Vec<_> =
                            output.blocked_scopes.iter().map(|s| format!("  {s}")).collect();
                        display.sort();
                        (
                            format!(
                                "[BLOCKED] Review not approved. Required scopes:\n{}",
                                display.join("\n")
                            ),
                            1u8,
                        )
                    }
                };
                CommandOutcome { stdout: None, stderr: Some(msg), exit_code }
            }
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn review_check_zero_findings(&self, query: ReviewCheckZeroFindingsQuery) -> CommandOutcome {
        check_zero_findings_result_to_command_outcome(
            self.check_zero_findings.check_zero_findings(&query),
        )
    }

    fn review_results(&self, input: ReviewResultsInput) -> CommandOutcome {
        let (track_id, items_dir, request, limit, round_type, no_hint) = input.into_parts();
        match self.results.results(track_id, items_dir, request) {
            Ok(output) => CommandOutcome::success(Some(render_review_results(
                output,
                limit,
                &round_type,
                no_hint,
            ))),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn review_classify(
        &self,
        paths: Vec<String>,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> CommandOutcome {
        match self.service.classify(paths, track_id, items_dir) {
            Ok(entries) => {
                let output: String = entries
                    .into_iter()
                    .map(|(path, scopes)| format!("{path}\t{scopes}\n"))
                    .collect();
                CommandOutcome::success(Some(output))
            }
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn review_files(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> CommandOutcome {
        match self.service.files(scope, track_id, items_dir) {
            Ok(files) => {
                let output: String = files.into_iter().map(|f| format!("{f}\n")).collect();
                CommandOutcome::success(Some(output))
            }
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn review_validate_scope(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> CommandOutcome {
        match self.service.validate_scope(scope, track_id, items_dir) {
            Ok(()) => CommandOutcome::success(None),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn review_get_briefing(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> CommandOutcome {
        match self.service.get_briefing(scope, track_id, items_dir) {
            Ok(maybe_path) => CommandOutcome::success(maybe_path),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn review_persist_commit_hash(
        &self,
        track_id: String,
        workspace_root: PathBuf,
    ) -> CommandOutcome {
        match self.service.persist_commit_hash(track_id, workspace_root) {
            Ok(sha) => CommandOutcome {
                stdout: None,
                stderr: Some(format!("[review] Recorded .commit_hash: {sha}")),
                exit_code: 0,
            },
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }
}

/// Driving adapter for review-fix invocations.
///
/// Composition supplies the injected [`RunReviewFixService`]. Callers supply
/// raw delivery values, which this primary adapter validates through the
/// usecase-owned command constructor.
pub struct ReviewFixDriver {
    service: Arc<dyn RunReviewFixService>,
}

impl ReviewFixDriver {
    /// Creates a review-fix driver from composition-owned wiring.
    #[must_use]
    pub fn new(service: Arc<dyn RunReviewFixService>) -> Self {
        Self { service }
    }

    /// Validates and executes the injected interactor, then renders its protocol.
    #[must_use]
    pub fn handle(&self, input: ReviewFixInput) -> CommandOutcome {
        let (scope, briefing_file, explicit_track_id, items_dir, round_type, model) =
            input.into_parts();
        let request = match RunReviewFixRequest::try_new(
            scope,
            briefing_file,
            explicit_track_id,
            items_dir,
            round_type,
            model,
        ) {
            Ok(request) => request,
            Err(error) => return CommandOutcome::failure(Some(error.to_string())),
        };
        match self.service.run(request) {
            Ok(output) => review_fix_output_to_outcome(output),
            Err(RunReviewFixError::FixRunnerFailed(
                ReviewFixRunnerError::SubagentDispatchRequired(instruction),
            )) => subagent_dispatch_to_outcome(*instruction),
            // A wrapped smoke-test failure is a preflight failure (not a review outcome).
            // Preserve exit 2 + diagnostic on stderr without emitting a
            // `REVIEW_FIX_STATUS:` line so orchestrators do not classify it
            // as a normal review-fix outcome.
            Err(RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::SmokeTestFailed(
                message,
            ))) => CommandOutcome {
                stdout: None,
                stderr: Some(format!("[ERROR] smoke test failed: {}", message.as_str())),
                exit_code: 2,
            },
            Err(error) => CommandOutcome::failure(Some(error.to_string())),
        }
    }
}

/// Raw review-fix delivery values supplied by the CLI.
#[derive(Debug, Clone)]
pub struct ReviewFixInput {
    scope: String,
    briefing_file: PathBuf,
    explicit_track_id: Option<String>,
    items_dir: PathBuf,
    round_type: String,
    model: Option<String>,
}

impl ReviewFixInput {
    #[must_use]
    pub fn new(
        scope: String,
        briefing_file: PathBuf,
        explicit_track_id: Option<String>,
        items_dir: PathBuf,
        round_type: String,
        model: Option<String>,
    ) -> Self {
        Self { scope, briefing_file, explicit_track_id, items_dir, round_type, model }
    }

    #[must_use]
    pub fn into_parts(self) -> (String, PathBuf, Option<String>, PathBuf, String, Option<String>) {
        (
            self.scope,
            self.briefing_file,
            self.explicit_track_id,
            self.items_dir,
            self.round_type,
            self.model,
        )
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn review_fix_output_to_outcome(output: RunReviewFixOutput) -> CommandOutcome {
    let exit_code = match output.exit_code {
        0 => 0,
        2 => 2,
        _ => 1,
    };
    CommandOutcome {
        stdout: Some(format!("REVIEW_FIX_STATUS: {}", output.status)),
        stderr: output.stderr,
        exit_code,
    }
}

fn run_review_output_to_outcome(out: usecase::review_v2::RunReviewOutput) -> CommandOutcome {
    if out.skipped {
        return CommandOutcome {
            stdout: Some(r#"{"verdict":"zero_findings","findings":[]}"#.to_owned()),
            stderr: None,
            exit_code: 0,
        };
    }
    // Preserve the underlying reviewer's exit code so the convention that
    // `findings_remain` returns exit 2 (distinguishing review findings from
    // subprocess failures) survives the cli_driver boundary.
    CommandOutcome { stdout: out.summary, stderr: None, exit_code: out.exit_code }
}

fn subagent_dispatch_to_outcome(instruction: SubagentDispatchInstruction) -> CommandOutcome {
    let Some(briefing_file) = instruction.briefing_file.to_str() else {
        return CommandOutcome::failure(Some(
            "review-fix dispatch briefing path is not valid UTF-8".to_owned(),
        ));
    };
    let Some(repository_root) = instruction.repository_root.to_str() else {
        return CommandOutcome::failure(Some(
            "review-fix dispatch repository root is not valid UTF-8".to_owned(),
        ));
    };
    let json = format!(
        "{{\"agent\":{},\"model\":{},\"effort\":{},\"scope\":{},\"briefing_file\":{},\"track_id\":{},\"repository_root\":{},\"round_type\":{}}}",
        json_str(instruction.agent.as_str()),
        json_str(instruction.model.as_str()),
        json_str(effort_value(instruction.effort)),
        json_str(instruction.scope.as_str()),
        json_str(briefing_file),
        json_str(instruction.track_id.as_str()),
        json_str(repository_root),
        json_str(match instruction.round_type {
            usecase::review_v2::ReviewRoundType::Fast => "fast",
            usecase::review_v2::ReviewRoundType::Final => "final",
        }),
    );
    CommandOutcome {
        stdout: Some(format!("{SUBAGENT_DISPATCH_SENTINEL}\n{json}")),
        stderr: None,
        exit_code: SUBAGENT_DISPATCH_EXIT_CODE,
    }
}

fn effort_value(effort: usecase::capability_exec::ReasoningEffort) -> &'static str {
    match effort {
        usecase::capability_exec::ReasoningEffort::Low => "low",
        usecase::capability_exec::ReasoningEffort::Medium => "medium",
        usecase::capability_exec::ReasoningEffort::High => "high",
        usecase::capability_exec::ReasoningEffort::XHigh => "xhigh",
        usecase::capability_exec::ReasoningEffort::Max => "max",
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    #[cfg(unix)]
    use std::ffi::OsString;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use usecase::capability_exec::{ModelName, ReasoningEffort};
    use usecase::git_workflow::DiagnosticText;
    use usecase::review_v2::run_review_fix::ReviewFixResolution;
    use usecase::review_v2::run_review_fix::{
        ReviewFixRunner, ReviewFixRunnerError, ReviewFixTrackResolveError,
        ReviewFixTrackResolverPort, ReviewTrackId, RunReviewFixCommand, RunReviewFixError,
        RunReviewFixInteractor, RunReviewFixOutput, RunReviewFixRequest, RunReviewFixService,
    };
    use usecase::review_v2::{
        NonEmptyReviewerFindingsOutput, ReviewApprovalOutput, ReviewCheckApprovedError,
        ReviewCheckZeroFindingsEvaluationError, ReviewCheckZeroFindingsOutcome,
        ReviewCheckZeroFindingsQuery, ReviewCheckZeroFindingsService,
        ReviewCheckZeroFindingsValidationError, ReviewRequiredReason, ReviewResultsError,
        ReviewResultsInteractor, ReviewResultsRoundPort, ReviewResultsScopePort,
        ReviewResultsScopeSnapshot, ReviewResultsService, ReviewResultsStatePort,
        ReviewRoundResultOutput, ReviewRoundResultVerdict, ReviewRoundType, ReviewRunInput,
        ReviewRunLocalOutput, ReviewScopeName, ReviewScopeSelectionRequest,
        ReviewScopeSelectionValidationError, ReviewService, ReviewStoredRound,
        ReviewStoredScopeState, ReviewStoredScopeStateEntry, ReviewerFindingOutput, RunReviewError,
        RunReviewOutput, SubagentDispatchInstruction, SubagentName,
    };

    use super::{
        ReviewCheckRoundSelect, ReviewCheckZeroFindingsInput, ReviewDriver, ReviewFixDriver,
        ReviewFixInput, ReviewInput, ReviewResultsInput, SUBAGENT_DISPATCH_EXIT_CODE,
        SUBAGENT_DISPATCH_SENTINEL, check_zero_findings_outcome_to_command_outcome,
        check_zero_findings_result_to_command_outcome, review_run_local_output_to_outcome,
        subagent_dispatch_to_outcome,
    };
    struct UnusedReviewService {
        results_service: Option<Arc<dyn ReviewResultsService>>,
    }

    impl ReviewService for UnusedReviewService {
        fn run_codex(
            &self,
            _input: usecase::review_v2::ReviewRunInput,
        ) -> Result<usecase::review_v2::RunReviewOutput, usecase::review_v2::RunReviewError>
        {
            panic!("check-zero-findings must not call the aggregate review service")
        }

        fn run_claude(
            &self,
            _input: usecase::review_v2::ReviewRunInput,
        ) -> Result<usecase::review_v2::RunReviewOutput, usecase::review_v2::RunReviewError>
        {
            panic!("check-zero-findings must not call the aggregate review service")
        }

        fn run_local(
            &self,
            _model: Option<String>,
            _timeout_seconds: u64,
            _briefing_file: Option<PathBuf>,
            _prompt: Option<String>,
            _track_id: Option<String>,
            _round_type: String,
            _group: String,
            _items_dir: PathBuf,
        ) -> usecase::review_v2::ReviewRunLocalOutput {
            panic!("check-zero-findings must not call the aggregate review service")
        }

        fn check_approved(
            &self,
            _track_id: String,
            _items_dir: PathBuf,
        ) -> Result<
            usecase::review_v2::ReviewApprovalOutput,
            usecase::review_v2::ReviewCheckApprovedError,
        > {
            panic!("check-zero-findings must not call the aggregate review service")
        }

        fn classify(
            &self,
            _paths: Vec<String>,
            _track_id: Option<String>,
            _items_dir: PathBuf,
        ) -> Result<Vec<(String, String)>, usecase::review_v2::ReviewAuxError> {
            panic!("check-zero-findings must not call the aggregate review service")
        }

        fn files(
            &self,
            _scope: String,
            _track_id: Option<String>,
            _items_dir: PathBuf,
        ) -> Result<Vec<String>, usecase::review_v2::ReviewAuxError> {
            panic!("check-zero-findings must not call the aggregate review service")
        }

        fn validate_scope(
            &self,
            _scope: String,
            _track_id: Option<String>,
            _items_dir: PathBuf,
        ) -> Result<(), usecase::review_v2::ReviewAuxError> {
            panic!("check-zero-findings must not call the aggregate review service")
        }

        fn get_briefing(
            &self,
            _scope: String,
            _track_id: Option<String>,
            _items_dir: PathBuf,
        ) -> Result<Option<String>, usecase::review_v2::ReviewAuxError> {
            panic!("check-zero-findings must not call the aggregate review service")
        }

        fn persist_commit_hash(
            &self,
            _track_id: String,
            _workspace_root: PathBuf,
        ) -> Result<String, usecase::commit_hash_persistence::CommitHashPersistenceError> {
            panic!("check-zero-findings must not call the aggregate review service")
        }
    }

    impl usecase::review_v2::ReviewResultsService for UnusedReviewService {
        fn results(
            &self,
            track_id: Option<String>,
            items_dir: PathBuf,
            request: usecase::review_v2::ReviewScopeSelectionRequest,
        ) -> Result<usecase::review_v2::ReviewResultsOutput, usecase::review_v2::ReviewResultsError>
        {
            self.results_service
                .as_ref()
                .expect("only the results test may invoke the aggregate service")
                .results(track_id, items_dir, request)
        }
    }

    struct CodexAggregateService {
        received: Mutex<Option<ReviewRunInput>>,
    }

    impl ReviewService for CodexAggregateService {
        fn run_codex(&self, input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError> {
            *self.received.lock().expect("capture lock is healthy") = Some(input);
            Ok(RunReviewOutput {
                verdict_kind: "approved".to_owned(),
                skipped: false,
                finding_count: 0,
                summary: Some("aggregate review completed".to_owned()),
                exit_code: 0,
            })
        }

        fn run_claude(&self, _input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError> {
            panic!("Codex aggregate test must not invoke Claude")
        }

        fn run_local(
            &self,
            _model: Option<String>,
            _timeout_seconds: u64,
            _briefing_file: Option<PathBuf>,
            _prompt: Option<String>,
            _track_id: Option<String>,
            _round_type: String,
            _group: String,
            _items_dir: PathBuf,
        ) -> usecase::review_v2::ReviewRunLocalOutput {
            panic!("Codex aggregate test must not invoke local review")
        }

        fn check_approved(
            &self,
            _track_id: String,
            _items_dir: PathBuf,
        ) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError> {
            panic!("Codex aggregate test must not check approval")
        }

        fn classify(
            &self,
            _paths: Vec<String>,
            _track_id: Option<String>,
            _items_dir: PathBuf,
        ) -> Result<Vec<(String, String)>, usecase::review_v2::ReviewAuxError> {
            panic!("Codex aggregate test must not classify")
        }

        fn files(
            &self,
            _scope: String,
            _track_id: Option<String>,
            _items_dir: PathBuf,
        ) -> Result<Vec<String>, usecase::review_v2::ReviewAuxError> {
            panic!("Codex aggregate test must not list files")
        }

        fn validate_scope(
            &self,
            _scope: String,
            _track_id: Option<String>,
            _items_dir: PathBuf,
        ) -> Result<(), usecase::review_v2::ReviewAuxError> {
            panic!("Codex aggregate test must not validate a scope")
        }

        fn get_briefing(
            &self,
            _scope: String,
            _track_id: Option<String>,
            _items_dir: PathBuf,
        ) -> Result<Option<String>, usecase::review_v2::ReviewAuxError> {
            panic!("Codex aggregate test must not load a briefing")
        }

        fn persist_commit_hash(
            &self,
            _track_id: String,
            _workspace_root: PathBuf,
        ) -> Result<String, usecase::commit_hash_persistence::CommitHashPersistenceError> {
            panic!("Codex aggregate test must not persist a commit hash")
        }
    }

    impl usecase::review_v2::ReviewResultsService for CodexAggregateService {
        fn results(
            &self,
            _track_id: Option<String>,
            _items_dir: PathBuf,
            _request: usecase::review_v2::ReviewScopeSelectionRequest,
        ) -> Result<usecase::review_v2::ReviewResultsOutput, usecase::review_v2::ReviewResultsError>
        {
            panic!("Codex aggregate test must not render results")
        }
    }

    struct CapturingCheckZeroFindingsService {
        received_query: Mutex<Option<ReviewCheckZeroFindingsQuery>>,
        result: Mutex<
            Option<Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsEvaluationError>>,
        >,
    }

    impl CapturingCheckZeroFindingsService {
        fn new(
            result: Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsEvaluationError>,
        ) -> Self {
            Self { received_query: Mutex::new(None), result: Mutex::new(Some(result)) }
        }

        fn received_query(&self) -> Option<ReviewCheckZeroFindingsQuery> {
            self.received_query.lock().expect("capture lock is healthy").clone()
        }
    }

    impl ReviewCheckZeroFindingsService for CapturingCheckZeroFindingsService {
        fn check_zero_findings(
            &self,
            query: &ReviewCheckZeroFindingsQuery,
        ) -> Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsEvaluationError>
        {
            let mut received = self.received_query.lock().expect("capture lock is healthy");
            assert!(received.is_none(), "driver must call focused service once");
            *received = Some(query.clone());
            self.result
                .lock()
                .expect("result lock is healthy")
                .take()
                .expect("driver must not call focused service twice")
        }
    }

    fn check_zero_findings_query() -> ReviewCheckZeroFindingsQuery {
        ReviewCheckZeroFindingsQuery::try_new(
            PathBuf::from("track/items"),
            "review-driver-check-2026".to_owned(),
            "cli_driver".to_owned(),
        )
        .expect("valid test query")
    }

    fn check_zero_findings_input() -> ReviewCheckZeroFindingsInput {
        ReviewCheckZeroFindingsInput::try_new(
            PathBuf::from("track/items"),
            "review-driver-check-2026".to_owned(),
            "cli_driver".to_owned(),
            ReviewCheckRoundSelect::Final,
        )
        .expect("valid driver input")
    }

    fn review_driver_for_check(
        result: Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsEvaluationError>,
    ) -> (ReviewDriver, Arc<CapturingCheckZeroFindingsService>) {
        let focused_service = Arc::new(CapturingCheckZeroFindingsService::new(result));
        let driver = ReviewDriver::new(
            Arc::new(UnusedReviewService { results_service: None }),
            Arc::new(UnusedReviewService { results_service: None }),
            focused_service.clone(),
        );
        (driver, focused_service)
    }

    #[test]
    fn test_review_driver_hands_codex_input_to_injected_aggregate_service() {
        let service = Arc::new(CodexAggregateService { received: Mutex::new(None) });
        let driver = ReviewDriver::new(
            service.clone(),
            Arc::new(UnusedReviewService { results_service: None }),
            Arc::new(CapturingCheckZeroFindingsService::new(Ok(
                ReviewCheckZeroFindingsOutcome::CurrentFinalZeroFindings,
            ))),
        );

        let outcome = driver.handle(ReviewInput::RunCodex(
            "gpt-5.5".to_owned(),
            90,
            Some(PathBuf::from("briefing.md")),
            Some("review the boundary".to_owned()),
            Some("aggregate-handoff-2026".to_owned()),
            "final".to_owned(),
            "cli_driver".to_owned(),
            PathBuf::from("track/items"),
        ));

        let received = service
            .received
            .lock()
            .expect("capture lock is healthy")
            .take()
            .expect("driver must invoke the aggregate service once");
        assert_eq!(received.model, "gpt-5.5");
        assert_eq!(received.timeout_seconds, 90);
        assert_eq!(received.briefing_file, Some(PathBuf::from("briefing.md")));
        assert_eq!(received.prompt.as_deref(), Some("review the boundary"));
        assert_eq!(received.track_id.as_deref(), Some("aggregate-handoff-2026"));
        assert_eq!(received.round_type, "final");
        assert_eq!(received.group, "cli_driver");
        assert_eq!(received.items_dir, PathBuf::from("track/items"));
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("aggregate review completed"));
    }

    #[test]
    fn test_review_run_local_output_renders_structured_diagnostics_to_stderr() {
        let outcome = review_run_local_output_to_outcome(ReviewRunLocalOutput {
            summary: Some("review completed".to_owned()),
            diagnostics: vec![
                DiagnosticText::new("[info] provider=codex"),
                DiagnosticText::new("[warn] briefing omitted"),
            ],
            exit_code: 0,
        });

        assert_eq!(outcome.stdout.as_deref(), Some("review completed"));
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("[info] provider=codex\n[warn] briefing omitted")
        );
        assert_eq!(outcome.exit_code, 0);
    }

    #[test]
    fn test_review_run_local_output_with_empty_diagnostics_emits_no_stderr() {
        let outcome = review_run_local_output_to_outcome(ReviewRunLocalOutput {
            summary: None,
            diagnostics: Vec::new(),
            exit_code: 17,
        });

        assert_eq!(outcome.stdout, None);
        assert_eq!(outcome.stderr, None);
        assert_eq!(outcome.exit_code, 17);
    }

    #[test]
    fn test_review_driver_check_zero_findings_dispatches_success_to_focused_service() {
        let query = check_zero_findings_query();
        let input = check_zero_findings_input();
        let (driver, focused_service) =
            review_driver_for_check(Ok(ReviewCheckZeroFindingsOutcome::CurrentFinalZeroFindings));

        let outcome = driver.handle(ReviewInput::CheckZeroFindings(input));

        assert_eq!(focused_service.received_query(), Some(query));
        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stderr.is_none());
    }

    #[test]
    fn test_review_driver_check_zero_findings_dispatches_non_success_to_focused_service() {
        for result in [
            Ok(ReviewCheckZeroFindingsOutcome::MissingFinalVerdict),
            Ok(ReviewCheckZeroFindingsOutcome::StaleFinalVerdict),
            Ok(ReviewCheckZeroFindingsOutcome::FindingsRemain),
        ] {
            let query = check_zero_findings_query();
            let input = check_zero_findings_input();
            let (driver, focused_service) = review_driver_for_check(result);

            let outcome = driver.handle(ReviewInput::CheckZeroFindings(input));

            assert_eq!(focused_service.received_query(), Some(query));
            assert_ne!(outcome.exit_code, 0);
            assert!(outcome.stderr.is_some());
        }
    }

    #[test]
    fn test_review_check_zero_findings_input_converts_named_and_other_scopes() {
        let named = check_zero_findings_input().into_query();
        assert_eq!(named, check_zero_findings_query());

        let other = ReviewCheckZeroFindingsInput::try_new(
            PathBuf::from("track/items"),
            "review-driver-check-2026".to_owned(),
            "Other".to_owned(),
            ReviewCheckRoundSelect::Final,
        )
        .expect("other input converts")
        .into_query();
        let expected_other = ReviewCheckZeroFindingsQuery::try_new(
            PathBuf::from("track/items"),
            "review-driver-check-2026".to_owned(),
            "other".to_owned(),
        )
        .expect("other query is valid");
        assert_eq!(other, expected_other);
    }

    #[test]
    fn test_review_check_zero_findings_input_forwards_raw_tokens_to_usecase_query() {
        let items_dir = PathBuf::from("track/items");
        let track_id = "review-driver-check-2026".to_owned();
        let scope = "cli_driver".to_owned();
        let input = ReviewCheckZeroFindingsInput::try_new(
            items_dir.clone(),
            track_id.clone(),
            scope.clone(),
            ReviewCheckRoundSelect::Final,
        )
        .expect("valid raw input");
        let expected = ReviewCheckZeroFindingsQuery::try_new(items_dir, track_id, scope)
            .expect("valid usecase query");

        assert_eq!(input.clone().into_query(), expected);
        let ReviewCheckZeroFindingsInput(stored_query) = input;
        assert_eq!(stored_query, expected);
    }

    #[test]
    fn test_review_driver_manifest_has_no_production_domain_dependency() {
        let manifest: toml::Value =
            toml::from_str(include_str!("../Cargo.toml")).expect("cli-driver manifest must parse");
        let tables = manifest.as_table().expect("manifest must be a table");

        for table_name in ["dependencies", "build-dependencies"] {
            let contains_domain = tables
                .get(table_name)
                .and_then(toml::Value::as_table)
                .is_some_and(|dependencies| dependencies.contains_key("domain"));
            assert!(!contains_domain, "domain must not be a production {table_name} dependency");
        }

        if let Some(targets) = tables.get("target").and_then(toml::Value::as_table) {
            for (target_name, target) in targets {
                let target_tables =
                    target.as_table().expect("target dependency configuration must be a table");
                for table_name in ["dependencies", "build-dependencies"] {
                    let contains_domain = target_tables
                        .get(table_name)
                        .and_then(toml::Value::as_table)
                        .is_some_and(|dependencies| dependencies.contains_key("domain"));
                    assert!(
                        !contains_domain,
                        "domain must not be a production {table_name} dependency for {target_name}"
                    );
                }
            }
        }

        let dev_dependencies = tables
            .get("dev-dependencies")
            .and_then(toml::Value::as_table)
            .expect("dev-dependencies must be a table");
        assert!(dev_dependencies.contains_key("domain"));
    }

    #[test]
    fn test_review_v2_sources_do_not_reexport_domain_scope_name() {
        let review_v2_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../libs/usecase/src/review_v2");
        let forbidden_reexport = ["pub use domain::review_v2::{", "ScopeName"].concat();

        for entry in fs::read_dir(review_v2_dir).expect("review_v2 source directory must exist") {
            let path = entry.expect("review_v2 directory entry must be readable").path();
            if path.extension().is_some_and(|extension| extension == "rs") {
                let source = fs::read_to_string(&path).expect("review_v2 source must be readable");
                assert!(
                    !source.contains(&forbidden_reexport),
                    "{} must not re-export domain ScopeName",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn test_review_check_zero_findings_input_rejects_invalid_scope_name() {
        let result = ReviewCheckZeroFindingsInput::try_new(
            PathBuf::from("track/items"),
            "review-driver-check-2026".to_owned(),
            "".to_owned(),
            ReviewCheckRoundSelect::Final,
        );
        assert!(matches!(result, Err(ReviewCheckZeroFindingsValidationError::InvalidScope(_))));
    }

    #[test]
    fn test_review_results_input_valid_selector_converts_to_usecase_selection() {
        let input = ReviewResultsInput::try_new(
            Some("review-driver-results-2026".to_owned()),
            PathBuf::from("track/items"),
            Some("cli_driver".to_owned()),
            false,
            0,
            "any".to_owned(),
            false,
        )
        .expect("valid delivery values must convert");

        let (track_id, items_dir, selection, limit, round_type, no_hint) = input.into_parts();
        assert_eq!(track_id.as_deref(), Some("review-driver-results-2026"));
        assert_eq!(items_dir, PathBuf::from("track/items"));
        assert!(
            matches!(selection, ReviewScopeSelectionRequest::NamedCandidate(scope) if scope.as_str() == "cli_driver")
        );
        assert_eq!(limit, 0);
        assert_eq!(round_type, "any");
        assert!(!no_hint);
    }

    #[test]
    fn test_review_results_input_omitted_and_explicit_all_convert_to_all_selection() {
        let omitted = ReviewResultsInput::try_new(
            None,
            PathBuf::from("track/items"),
            None,
            false,
            0,
            "any".to_owned(),
            false,
        )
        .expect("omitting both selectors must select all scopes");
        let explicit_all = ReviewResultsInput::try_new(
            None,
            PathBuf::from("track/items"),
            None,
            true,
            0,
            "any".to_owned(),
            false,
        )
        .expect("an explicit all selector must select all scopes");

        assert!(matches!(omitted.into_parts().2, ReviewScopeSelectionRequest::All));
        assert!(matches!(explicit_all.into_parts().2, ReviewScopeSelectionRequest::All));
    }

    #[test]
    fn test_review_results_input_scope_and_all_returns_usecase_error() {
        let result = ReviewResultsInput::try_new(
            None,
            PathBuf::from("track/items"),
            Some("cli_driver".to_owned()),
            true,
            0,
            "any".to_owned(),
            false,
        );

        assert!(matches!(result, Err(ReviewScopeSelectionValidationError::ScopeAndAll)));
    }

    #[test]
    fn test_review_results_input_invalid_scope_returns_usecase_error() {
        let result = ReviewResultsInput::try_new(
            None,
            PathBuf::from("track/items"),
            Some("".to_owned()),
            false,
            0,
            "any".to_owned(),
            false,
        );

        assert!(matches!(result, Err(ReviewScopeSelectionValidationError::InvalidScope(_))));
    }

    #[test]
    fn test_review_results_driver_renders_real_interactor_round_details_and_limited_history() {
        struct ScopePort;

        impl ReviewResultsScopePort for ScopePort {
            fn load_scope_snapshot(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<ReviewResultsScopeSnapshot, ReviewResultsError> {
                Ok(ReviewResultsScopeSnapshot {
                    base: "origin/main".to_owned(),
                    configured_scopes: vec![
                        ReviewScopeName::try_new("cli_driver".to_owned())
                            .expect("fixed test scope must be valid"),
                    ],
                    review_json_exists: true,
                })
            }
        }

        struct StatePort;

        impl ReviewResultsStatePort for StatePort {
            fn load_scope_states(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<Vec<ReviewStoredScopeStateEntry>, ReviewResultsError> {
                Ok(vec![ReviewStoredScopeStateEntry {
                    scope: ReviewScopeName::try_new("cli_driver".to_owned())
                        .expect("fixed test scope must be valid"),
                    state: ReviewStoredScopeState::Required(ReviewRequiredReason::FindingsRemain),
                }])
            }
        }

        struct RoundPort;

        impl ReviewResultsRoundPort for RoundPort {
            fn load_scope_rounds(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
                scope: &ReviewScopeName,
            ) -> Result<Vec<ReviewStoredRound>, ReviewResultsError> {
                assert_eq!(scope.as_str(), "cli_driver");
                Ok(vec![
                    ReviewRoundResultOutput {
                        round_type: ReviewRoundType::Fast,
                        at: "2026-08-10T12:00:00Z".to_owned(),
                        verdict: ReviewRoundResultVerdict::ZeroFindings,
                    },
                    ReviewRoundResultOutput {
                        round_type: ReviewRoundType::Fast,
                        at: "2026-08-10T12:01:00Z".to_owned(),
                        verdict: ReviewRoundResultVerdict::ZeroFindings,
                    },
                    ReviewRoundResultOutput {
                        round_type: ReviewRoundType::Final,
                        at: "2026-08-10T12:02:00Z".to_owned(),
                        verdict: ReviewRoundResultVerdict::FindingsRemain(
                            NonEmptyReviewerFindingsOutput::try_new(vec![ReviewerFindingOutput {
                                message: usecase::git_workflow::DiagnosticText::new(
                                    "retained finding detail",
                                ),
                                severity: Some("P1".to_owned()),
                                file: Some("apps/cli-driver/src/review.rs".to_owned()),
                                line: Some(123),
                                category: Some("correctness".to_owned()),
                            }])
                            .expect("one test finding is non-empty"),
                        ),
                    },
                ])
            }
        }

        let results_service: Arc<dyn ReviewResultsService> =
            Arc::new(ReviewResultsInteractor::new(
                Arc::new(ScopePort),
                Arc::new(StatePort),
                Arc::new(RoundPort),
            ));
        let driver = ReviewDriver::new(
            Arc::new(UnusedReviewService { results_service: None }),
            results_service,
            Arc::new(CapturingCheckZeroFindingsService::new(Ok(
                ReviewCheckZeroFindingsOutcome::CurrentFinalZeroFindings,
            ))),
        );
        let input = ReviewResultsInput::try_new(
            Some("results-render-2026".to_owned()),
            PathBuf::from("track/items"),
            None,
            false,
            2,
            "any".to_owned(),
            true,
        )
        .expect("valid results input");

        let outcome = driver.handle(ReviewInput::Results(input));

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stderr, None);
        let rendered = outcome.stdout.expect("results must render to stdout");
        assert!(rendered.contains("final@2026-08-10T12:02:00Z findings_remain"));
        assert!(rendered.contains("retained finding detail (apps/cli-driver/src/review.rs:123)"));
        assert!(rendered.contains("history (newer first, up to --limit):"));
        assert!(rendered.contains("fast@2026-08-10T12:01:00Z zero_findings"));
        assert!(
            !rendered.contains("2026-08-10T12:00:00Z"),
            "--limit must truncate the oldest history round"
        );
    }

    #[test]
    fn test_review_results_driver_does_not_render_when_results_service_fails() {
        struct FailingResultsService;

        impl ReviewResultsService for FailingResultsService {
            fn results(
                &self,
                _track_id: Option<String>,
                _items_dir: PathBuf,
                _request: ReviewScopeSelectionRequest,
            ) -> Result<usecase::review_v2::ReviewResultsOutput, ReviewResultsError> {
                Err(ReviewResultsError::Failed(DiagnosticText::new(
                    "review results state could not be read",
                )))
            }
        }

        let driver = ReviewDriver::new(
            Arc::new(UnusedReviewService { results_service: None }),
            Arc::new(FailingResultsService),
            Arc::new(CapturingCheckZeroFindingsService::new(Ok(
                ReviewCheckZeroFindingsOutcome::CurrentFinalZeroFindings,
            ))),
        );
        let input = ReviewResultsInput::try_new(
            Some("results-render-failure-2026".to_owned()),
            PathBuf::from("track/items"),
            None,
            false,
            0,
            "any".to_owned(),
            true,
        )
        .expect("valid results input");

        let outcome = driver.handle(ReviewInput::Results(input));

        assert_ne!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, None);
        assert_eq!(outcome.stderr.as_deref(), Some("review results state could not be read"));
    }

    #[test]
    fn test_review_results_driver_renders_unknown_scope_membership_error() {
        struct UnknownScopeResultsService;

        impl ReviewResultsService for UnknownScopeResultsService {
            fn results(
                &self,
                _track_id: Option<String>,
                _items_dir: PathBuf,
                _request: ReviewScopeSelectionRequest,
            ) -> Result<usecase::review_v2::ReviewResultsOutput, ReviewResultsError> {
                Err(ReviewResultsError::UnknownScope(
                    ReviewScopeName::try_new("not-configured".to_owned())
                        .expect("format-valid scope"),
                ))
            }
        }

        let driver = ReviewDriver::new(
            Arc::new(UnusedReviewService { results_service: None }),
            Arc::new(UnknownScopeResultsService),
            Arc::new(CapturingCheckZeroFindingsService::new(Ok(
                ReviewCheckZeroFindingsOutcome::CurrentFinalZeroFindings,
            ))),
        );
        let input = ReviewResultsInput::try_new(
            Some("results-unknown-scope-2026".to_owned()),
            PathBuf::from("track/items"),
            Some("not-configured".to_owned()),
            false,
            0,
            "any".to_owned(),
            true,
        )
        .expect("valid results input");

        let outcome = driver.handle(ReviewInput::Results(input));

        assert_ne!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, None);
        assert_eq!(outcome.stderr.as_deref(), Some("unknown review results scope: not-configured"));
    }

    #[test]
    fn test_review_fix_input_preserves_raw_values_until_usecase_validation() {
        let input = ReviewFixInput::new(
            "  invalid scope  ".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/raw briefing.md"),
            Some(" Invalid Track Id ".to_owned()),
            PathBuf::from("track/items"),
            " later ".to_owned(),
            Some(" ".to_owned()),
        );

        let (scope, briefing_file, track_id, items_dir, round_type, model) = input.into_parts();
        assert_eq!(scope, "  invalid scope  ");
        assert_eq!(briefing_file, PathBuf::from("tmp/reviewer-runtime/raw briefing.md"));
        assert_eq!(track_id.as_deref(), Some(" Invalid Track Id "));
        assert_eq!(items_dir, PathBuf::from("track/items"));
        assert_eq!(round_type, " later ");
        assert_eq!(model.as_deref(), Some(" "));
    }

    #[test]
    fn test_review_check_zero_findings_current_final_zero_findings_returns_success() {
        let outcome = check_zero_findings_outcome_to_command_outcome(
            ReviewCheckZeroFindingsOutcome::CurrentFinalZeroFindings,
        );

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stderr.is_none());
    }

    #[test]
    fn test_review_check_zero_findings_missing_final_verdict_returns_nonzero_exit() {
        let outcome = check_zero_findings_outcome_to_command_outcome(
            ReviewCheckZeroFindingsOutcome::MissingFinalVerdict,
        );

        assert_ne!(outcome.exit_code, 0);
        assert!(outcome.stderr.is_some());
    }

    #[test]
    fn test_review_check_zero_findings_stale_final_verdict_returns_nonzero_exit() {
        let outcome = check_zero_findings_outcome_to_command_outcome(
            ReviewCheckZeroFindingsOutcome::StaleFinalVerdict,
        );

        assert_ne!(outcome.exit_code, 0);
        assert!(outcome.stderr.is_some());
    }

    #[test]
    fn test_review_check_zero_findings_findings_remain_returns_nonzero_exit() {
        let outcome = check_zero_findings_outcome_to_command_outcome(
            ReviewCheckZeroFindingsOutcome::FindingsRemain,
        );

        assert_ne!(outcome.exit_code, 0);
        assert!(outcome.stderr.is_some());
    }

    #[test]
    fn test_review_check_zero_findings_evaluation_failure_returns_nonzero_exit() {
        let outcome = check_zero_findings_result_to_command_outcome(Err(
            ReviewCheckZeroFindingsEvaluationError::EvaluationFailed(
                usecase::git_workflow::DiagnosticText::new("review.json is malformed"),
            ),
        ));

        assert_ne!(outcome.exit_code, 0);
        assert!(outcome.stderr.as_deref().is_some_and(|message| message.contains("malformed")));
    }

    #[test]
    fn test_review_driver_check_zero_findings_service_evaluation_failure_returns_nonzero_exit() {
        let query = check_zero_findings_query();
        let (driver, focused_service) =
            review_driver_for_check(Err(ReviewCheckZeroFindingsEvaluationError::EvaluationFailed(
                usecase::git_workflow::DiagnosticText::new("review artifact could not be read"),
            )));

        let outcome = driver.handle(ReviewInput::CheckZeroFindings(check_zero_findings_input()));

        assert_eq!(focused_service.received_query(), Some(query));
        assert_ne!(outcome.exit_code, 0);
        assert!(
            outcome.stderr.as_deref().is_some_and(|message| message.contains("could not be read"))
        );
    }

    #[test]
    fn test_review_check_zero_findings_unconfigured_scope_returns_nonzero_exit() {
        let outcome = check_zero_findings_outcome_to_command_outcome(
            ReviewCheckZeroFindingsOutcome::MissingFinalVerdict,
        );

        assert_ne!(outcome.exit_code, 0);
        assert!(outcome.stderr.is_some());
    }

    struct CompletedFixService;

    impl RunReviewFixService for CompletedFixService {
        fn run(
            &self,
            _command: RunReviewFixRequest,
        ) -> Result<RunReviewFixOutput, RunReviewFixError> {
            Ok(RunReviewFixOutput { status: "completed".to_owned(), exit_code: 0, stderr: None })
        }
    }

    #[test]
    fn test_review_fix_driver_completed_renders_status() {
        let driver = ReviewFixDriver::new(Arc::new(CompletedFixService));
        let outcome = driver.handle(ReviewFixInput::new(
            "cli_driver".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            Some("review-fix-driver-2026".to_owned()),
            PathBuf::from("track/items"),
            "fast".to_owned(),
            Some("gpt-5.5".to_owned()),
        ));

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("REVIEW_FIX_STATUS: completed"));
        assert_eq!(outcome.stderr, None);
    }

    struct DispatchingFixService;

    impl RunReviewFixService for DispatchingFixService {
        fn run(
            &self,
            _request: RunReviewFixRequest,
        ) -> Result<RunReviewFixOutput, RunReviewFixError> {
            Err(RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::SubagentDispatchRequired(
                Box::new(SubagentDispatchInstruction {
                    agent: SubagentName::try_new("review-fix-lead".to_owned())
                        .expect("valid test subagent"),
                    model: ModelName::try_new("gpt-5.5".to_owned()).expect("valid test model"),
                    effort: ReasoningEffort::High,
                    scope: ReviewScopeName::try_new("cli_driver".to_owned())
                        .expect("valid test review scope"),
                    briefing_file: PathBuf::from("tmp/reviewer-runtime/briefing-cli_driver.md"),
                    track_id: ReviewTrackId::try_new("dispatch-driver-2026".to_owned())
                        .expect("valid test track ID"),
                    repository_root: PathBuf::from("/resolver-proven/root"),
                    round_type: ReviewRoundType::Final,
                }),
            )))
        }
    }

    #[test]
    fn test_review_fix_driver_renders_validated_dispatch_instruction_protocol() {
        let outcome =
            ReviewFixDriver::new(Arc::new(DispatchingFixService)).handle(ReviewFixInput::new(
                "cli_driver".to_owned(),
                PathBuf::from("tmp/reviewer-runtime/briefing.md"),
                None,
                PathBuf::from("track/items"),
                "final".to_owned(),
                None,
            ));

        assert_eq!(outcome.exit_code, SUBAGENT_DISPATCH_EXIT_CODE);
        assert_eq!(outcome.stderr, None);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "SUBAGENT_DISPATCH_REQUIRED\n{\"agent\":\"review-fix-lead\",\"model\":\"gpt-5.5\",\"effort\":\"high\",\"scope\":\"cli_driver\",\"briefing_file\":\"tmp/reviewer-runtime/briefing-cli_driver.md\",\"track_id\":\"dispatch-driver-2026\",\"repository_root\":\"/resolver-proven/root\",\"round_type\":\"final\"}"
            )
        );
    }

    struct TrackMismatchFixService;

    impl RunReviewFixService for TrackMismatchFixService {
        fn run(
            &self,
            _request: RunReviewFixRequest,
        ) -> Result<RunReviewFixOutput, RunReviewFixError> {
            Err(RunReviewFixError::TrackMismatch {
                explicit: ReviewTrackId::try_new("requested-track-2026".to_owned())
                    .expect("fixed explicit track ID must be valid"),
                resolved: ReviewTrackId::try_new("current-track-2026".to_owned())
                    .expect("fixed resolved track ID must be valid"),
            })
        }
    }

    #[test]
    fn test_review_fix_driver_renders_typed_track_mismatch_as_nonzero_outcome() {
        let outcome =
            ReviewFixDriver::new(Arc::new(TrackMismatchFixService)).handle(ReviewFixInput::new(
                "cli_driver".to_owned(),
                PathBuf::from("tmp/reviewer-runtime/briefing.md"),
                Some("requested-track-2026".to_owned()),
                PathBuf::from("track/items"),
                "fast".to_owned(),
                None,
            ));

        assert_ne!(outcome.exit_code, 0);
        assert!(outcome.stdout.is_none());
        assert!(outcome.stderr.as_deref().is_some_and(|message| {
            message.contains("explicit track 'requested-track-2026' does not match current branch track 'current-track-2026'")
        }));
    }

    #[cfg(any())]
    #[derive(Clone, Copy)]
    enum BriefingLoadFailure {
        UntrustedFile,
        ReadFailed,
        InvalidContent,
    }

    #[cfg(any())]
    struct BriefingLoadFailingFixService {
        failure: BriefingLoadFailure,
    }

    #[cfg(any())]
    impl RunReviewFixService for BriefingLoadFailingFixService {
        fn run(
            &self,
            _request: RunReviewFixRequest,
        ) -> Result<RunReviewFixOutput, RunReviewFixError> {
            let error = match self.failure {
                BriefingLoadFailure::UntrustedFile => {
                    ReviewFixBriefingLoadError::UntrustedFile(DiagnosticText::new("traversal"))
                }
                BriefingLoadFailure::ReadFailed => {
                    ReviewFixBriefingLoadError::ReadFailed(DiagnosticText::new("missing"))
                }
                BriefingLoadFailure::InvalidContent => ReviewFixBriefingLoadError::InvalidContent(
                    SubagentBriefingContentValidationError::ExceedsMaximumBytes,
                ),
            };
            Err(RunReviewFixError::BriefingLoad(error))
        }
    }

    #[cfg(any())]
    #[test]
    fn test_review_fix_driver_briefing_load_failures_render_nonzero_typed_diagnostics() {
        let cases = [
            (BriefingLoadFailure::UntrustedFile, &["not trusted: traversal"][..]),
            (BriefingLoadFailure::ReadFailed, &["could not read review-fix briefing: missing"][..]),
            (
                BriefingLoadFailure::InvalidContent,
                &[
                    "review-fix briefing content is invalid",
                    "review-fix briefing content exceeds the 65536-byte limit",
                ][..],
            ),
        ];

        for (failure, expected_details) in cases {
            let outcome = ReviewFixDriver::new(Arc::new(BriefingLoadFailingFixService { failure }))
                .handle(ReviewFixInput::new(
                    "cli_driver".to_owned(),
                    PathBuf::from("tmp/reviewer-runtime/briefing.md"),
                    None,
                    PathBuf::from("track/items"),
                    "final".to_owned(),
                    None,
                ));

            assert_ne!(outcome.exit_code, 0);
            assert!(outcome.stdout.is_none());
            assert!(outcome.stderr.as_deref().is_some_and(|message| {
                message.contains("briefing load failed")
                    && expected_details.iter().all(|detail| message.contains(detail))
            }));
        }
    }

    struct CapturingFixService {
        received: Mutex<bool>,
    }

    impl RunReviewFixService for CapturingFixService {
        fn run(
            &self,
            _request: RunReviewFixRequest,
        ) -> Result<RunReviewFixOutput, RunReviewFixError> {
            *self.received.lock().expect("test mutex must not be poisoned") = true;
            Ok(RunReviewFixOutput {
                status: "completed".to_owned(),
                exit_code: 0,
                stderr: Some("runner completed".to_owned()),
            })
        }
    }

    #[test]
    fn test_review_fix_driver_validates_raw_input_invokes_service_and_renders_outcome() {
        let service = Arc::new(CapturingFixService { received: Mutex::new(false) });
        let driver = ReviewFixDriver::new(service.clone());

        let outcome = driver.handle(ReviewFixInput::new(
            " cli_driver ".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            Some("review-fix-driver-2026".to_owned()),
            PathBuf::from("track/items"),
            "final".to_owned(),
            Some("gpt-5.5".to_owned()),
        ));

        assert!(*service.received.lock().expect("test mutex must not be poisoned"));
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("REVIEW_FIX_STATUS: completed"));
        assert_eq!(outcome.stderr.as_deref(), Some("runner completed"));
    }

    struct RecordingRenderFixService {
        received_requests: Mutex<Vec<RunReviewFixRequest>>,
    }

    impl RunReviewFixService for RecordingRenderFixService {
        fn run(
            &self,
            request: RunReviewFixRequest,
        ) -> Result<RunReviewFixOutput, RunReviewFixError> {
            let mut received_requests =
                self.received_requests.lock().expect("test mutex must not be poisoned");
            let first_request = received_requests.is_empty();
            received_requests.push(request);

            if first_request {
                Ok(RunReviewFixOutput {
                    status: "completed".to_owned(),
                    exit_code: 0,
                    stderr: Some("typed success diagnostic".to_owned()),
                })
            } else {
                Err(RunReviewFixError::TrackMismatch {
                    explicit: ReviewTrackId::try_new("requested-track-2026".to_owned())
                        .expect("fixed explicit track ID must be valid"),
                    resolved: ReviewTrackId::try_new("current-track-2026".to_owned())
                        .expect("fixed resolved track ID must be valid"),
                })
            }
        }
    }

    #[test]
    fn test_review_fix_driver_forwards_raw_input_to_usecase_and_renders_typed_results() {
        let service = Arc::new(RecordingRenderFixService { received_requests: Mutex::new(vec![]) });
        let driver = ReviewFixDriver::new(service.clone());

        let completed = driver.handle(ReviewFixInput::new(
            "cli_driver".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            Some("requested-track-2026".to_owned()),
            PathBuf::from("track/items"),
            "fast".to_owned(),
            Some("gpt-5.5".to_owned()),
        ));
        let track_mismatch = driver.handle(ReviewFixInput::new(
            "other".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/other-briefing.md"),
            None,
            PathBuf::from("track/items/other"),
            "final".to_owned(),
            None,
        ));

        assert_eq!(
            service.received_requests.lock().expect("test mutex must not be poisoned").len(),
            2,
            "each valid raw delivery input must reach the usecase service"
        );
        assert_eq!(completed.exit_code, 0);
        assert_eq!(completed.stdout.as_deref(), Some("REVIEW_FIX_STATUS: completed"));
        assert_eq!(completed.stderr.as_deref(), Some("typed success diagnostic"));
        assert_ne!(track_mismatch.exit_code, 0);
        assert!(track_mismatch.stdout.is_none());
        assert!(track_mismatch.stderr.as_deref().is_some_and(|message| {
            message.contains(
                "explicit track 'requested-track-2026' does not match current branch track 'current-track-2026'",
            )
        }));
    }

    struct RealInteractorResolver {
        received_items_dirs: Mutex<Vec<PathBuf>>,
    }

    #[cfg(any())]
    struct FixedBriefingLoader;

    #[cfg(any())]
    impl ReviewFixBriefingLoaderPort for FixedBriefingLoader {
        fn load_briefing_content(
            &self,
            _repository_root: &Path,
            _briefing_file: &Path,
        ) -> Result<SubagentBriefingContent, ReviewFixBriefingLoadError> {
            SubagentBriefingContent::try_new("briefing".to_owned())
                .map_err(ReviewFixBriefingLoadError::InvalidContent)
        }
    }

    #[cfg(any())]
    struct RecordingBriefingLoader {
        received_requests: Mutex<Vec<(PathBuf, PathBuf)>>,
        content: SubagentBriefingContent,
    }

    #[cfg(any())]
    impl ReviewFixBriefingLoaderPort for RecordingBriefingLoader {
        fn load_briefing_content(
            &self,
            repository_root: &Path,
            briefing_file: &Path,
        ) -> Result<SubagentBriefingContent, ReviewFixBriefingLoadError> {
            self.received_requests
                .lock()
                .expect("test mutex must not be poisoned")
                .push((repository_root.to_path_buf(), briefing_file.to_path_buf()));
            Ok(self.content.clone())
        }
    }

    impl ReviewFixTrackResolverPort for RealInteractorResolver {
        fn resolve_current_track(
            &self,
            items_dir: &Path,
        ) -> Result<ReviewFixResolution, ReviewFixTrackResolveError> {
            self.received_items_dirs
                .lock()
                .expect("test mutex must not be poisoned")
                .push(items_dir.to_path_buf());
            Ok(ReviewFixResolution::new(
                ReviewTrackId::try_new("driver-interactor-2026".to_owned())
                    .expect("fixed test track ID must be valid"),
                PathBuf::from("/test-repository"),
            ))
        }
    }

    struct RealInteractorRunner {
        received_commands: Mutex<Vec<RunReviewFixCommand>>,
    }

    impl ReviewFixRunner for RealInteractorRunner {
        fn run_fix(
            &self,
            command: RunReviewFixCommand,
        ) -> Result<RunReviewFixOutput, ReviewFixRunnerError> {
            self.received_commands.lock().expect("test mutex must not be poisoned").push(command);
            Ok(RunReviewFixOutput { status: "completed".to_owned(), exit_code: 0, stderr: None })
        }
    }

    #[test]
    fn test_review_fix_driver_real_interactor_delivers_briefing_path_and_renders_outcome() {
        let resolver =
            Arc::new(RealInteractorResolver { received_items_dirs: Mutex::new(Vec::new()) });
        let runner = Arc::new(RealInteractorRunner { received_commands: Mutex::new(Vec::new()) });
        let service: Arc<dyn RunReviewFixService> =
            Arc::new(RunReviewFixInteractor::new(resolver.clone(), runner.clone()));
        let driver = ReviewFixDriver::new(service);

        let outcome = driver.handle(ReviewFixInput::new(
            " cli_driver ".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            None,
            PathBuf::from("track/items/driver-interactor"),
            "final".to_owned(),
            Some("gpt-5.5".to_owned()),
        ));

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("REVIEW_FIX_STATUS: completed"));
        assert_eq!(outcome.stderr, None);
        assert_eq!(
            *resolver.received_items_dirs.lock().expect("test mutex must not be poisoned"),
            vec![PathBuf::from("track/items/driver-interactor")]
        );
        let command = runner
            .received_commands
            .lock()
            .expect("test mutex must not be poisoned")
            .pop()
            .expect("resolved command must reach the runner");
        assert_eq!(command.scope(), " cli_driver ");
        assert_eq!(command.track_id(), "driver-interactor-2026");
        assert_eq!(command.repository_root(), PathBuf::from("/test-repository"));
        assert_eq!(command.briefing_file(), Path::new("tmp/reviewer-runtime/briefing.md"));
        assert!(matches!(command.round_type(), ReviewRoundType::Final));
        assert_eq!(command.model().map(ModelName::as_str), Some("gpt-5.5"));
    }

    #[test]
    fn test_review_fix_driver_rejects_full_raw_validation_matrix_without_invoking_service() {
        let accepted_service = Arc::new(CapturingFixService { received: Mutex::new(false) });
        let accepted = ReviewFixDriver::new(accepted_service.clone()).handle(ReviewFixInput::new(
            "OtHeR".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            Some("review-fix-driver-2026".to_owned()),
            PathBuf::from("track/items"),
            "fast".to_owned(),
            Some("gpt-5.5".to_owned()),
        ));

        assert_eq!(accepted.exit_code, 0);
        assert!(
            *accepted_service.received.lock().expect("test mutex must not be poisoned"),
            "case-insensitive other must reach the usecase boundary"
        );

        let service = Arc::new(CapturingFixService { received: Mutex::new(false) });
        let driver = ReviewFixDriver::new(service.clone());
        let cases = [
            (
                "empty scope",
                "".to_owned(),
                Some("review-fix-driver-2026".to_owned()),
                "fast".to_owned(),
                "invalid review-fix scope",
            ),
            (
                "non-ASCII scope",
                "非ASCII".to_owned(),
                Some("review-fix-driver-2026".to_owned()),
                "fast".to_owned(),
                "invalid review-fix scope",
            ),
            (
                "track ID outside the domain invariant",
                "cli_driver".to_owned(),
                Some("Invalid Track ID".to_owned()),
                "fast".to_owned(),
                "invalid review-fix track ID",
            ),
            (
                "unknown round type",
                "cli_driver".to_owned(),
                Some("review-fix-driver-2026".to_owned()),
                "later".to_owned(),
                "invalid review-fix round type",
            ),
        ];
        for (case, scope, track_id, round_type, expected_field) in cases {
            let outcome = driver.handle(ReviewFixInput::new(
                scope,
                PathBuf::from("tmp/reviewer-runtime/briefing.md"),
                track_id,
                PathBuf::from("track/items"),
                round_type,
                Some("gpt-5.5".to_owned()),
            ));

            assert_ne!(outcome.exit_code, 0, "{case} must return a non-zero outcome");
            assert!(
                outcome
                    .stderr
                    .as_deref()
                    .is_some_and(|message| { message.contains(expected_field) }),
                "{case} must identify its invalid field"
            );
        }
        assert!(
            !*service.received.lock().expect("test mutex must not be poisoned"),
            "invalid raw input must be rejected before the injected service runs"
        );
    }

    #[test]
    fn test_review_fix_driver_renders_invalid_track_id_validation_error() {
        let service = Arc::new(CapturingFixService { received: Mutex::new(false) });
        let outcome = ReviewFixDriver::new(service.clone()).handle(ReviewFixInput::new(
            "cli_driver".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            Some("Invalid Track ID".to_owned()),
            PathBuf::from("track/items"),
            "fast".to_owned(),
            None,
        ));

        assert_ne!(outcome.exit_code, 0);
        assert!(outcome.stdout.is_none());
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|message| message.contains("invalid review-fix track ID"))
        );
        assert!(
            !*service.received.lock().expect("test mutex must not be poisoned"),
            "the Invalid(DiagnosticText) track-ID error must be rendered before service invocation"
        );
    }

    #[test]
    fn test_review_fix_driver_rejects_empty_explicit_track_id_with_focused_validation_error() {
        let service = Arc::new(CapturingFixService { received: Mutex::new(false) });
        let outcome = ReviewFixDriver::new(service.clone()).handle(ReviewFixInput::new(
            "cli_driver".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            Some(String::new()),
            PathBuf::from("track/items"),
            "fast".to_owned(),
            None,
        ));

        assert_ne!(outcome.exit_code, 0);
        assert!(outcome.stdout.is_none());
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|message| { message.contains("invalid review-fix track ID") })
        );
        assert!(
            !*service.received.lock().expect("test mutex must not be poisoned"),
            "an empty explicit track ID must be rejected before service invocation"
        );
    }

    #[test]
    fn test_review_fix_driver_renders_raw_validation_and_interactor_error_boundaries() {
        let accepted_service = Arc::new(CapturingFixService { received: Mutex::new(false) });
        let accepted = ReviewFixDriver::new(accepted_service.clone()).handle(ReviewFixInput::new(
            "OTHER".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            Some("review-fix-driver-2026".to_owned()),
            PathBuf::from("track/items"),
            "fast".to_owned(),
            None,
        ));
        assert_eq!(accepted.exit_code, 0);
        assert!(
            *accepted_service.received.lock().expect("test mutex must not be poisoned"),
            "catch-all other must not be a scope validation error"
        );

        let service = Arc::new(CapturingFixService { received: Mutex::new(false) });
        let driver = ReviewFixDriver::new(service.clone());

        let invalid_scope = driver.handle(ReviewFixInput::new(
            "".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            Some("review-fix-driver-2026".to_owned()),
            PathBuf::from("track/items"),
            "fast".to_owned(),
            None,
        ));
        let invalid_track = driver.handle(ReviewFixInput::new(
            "cli_driver".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            Some("invalid track id".to_owned()),
            PathBuf::from("track/items"),
            "fast".to_owned(),
            None,
        ));

        assert_ne!(invalid_scope.exit_code, 0);
        assert!(
            invalid_scope
                .stderr
                .as_deref()
                .is_some_and(|message| message.contains("invalid review-fix scope"))
        );
        assert_ne!(invalid_track.exit_code, 0);
        assert!(
            invalid_track
                .stderr
                .as_deref()
                .is_some_and(|message| message.contains("invalid review-fix track ID"))
        );
        assert!(
            !*service.received.lock().expect("test mutex must not be poisoned"),
            "invalid raw values must not invoke the injected service"
        );

        struct ResolvingTrack;

        impl ReviewFixTrackResolverPort for ResolvingTrack {
            fn resolve_current_track(
                &self,
                _items_dir: &Path,
            ) -> Result<ReviewFixResolution, ReviewFixTrackResolveError> {
                Ok(ReviewFixResolution::new(
                    ReviewTrackId::try_new("review-fix-driver-2026".to_owned())
                        .expect("fixed test track ID must be valid"),
                    PathBuf::from("/test-repository"),
                ))
            }
        }

        struct NonTrackBranch;

        impl ReviewFixTrackResolverPort for NonTrackBranch {
            fn resolve_current_track(
                &self,
                _items_dir: &Path,
            ) -> Result<ReviewFixResolution, ReviewFixTrackResolveError> {
                Err(ReviewFixTrackResolveError::NonTrackBranch(
                    usecase::git_workflow::DiagnosticText::new("detached"),
                ))
            }
        }

        struct SpawnFailingRunner;

        impl ReviewFixRunner for SpawnFailingRunner {
            fn run_fix(
                &self,
                _command: RunReviewFixCommand,
            ) -> Result<RunReviewFixOutput, ReviewFixRunnerError> {
                Err(ReviewFixRunnerError::SpawnFailed(usecase::git_workflow::DiagnosticText::new(
                    "runner unavailable",
                )))
            }
        }

        struct RootMismatchRunner;

        impl ReviewFixRunner for RootMismatchRunner {
            fn run_fix(
                &self,
                _command: RunReviewFixCommand,
            ) -> Result<RunReviewFixOutput, ReviewFixRunnerError> {
                Err(ReviewFixRunnerError::Unexpected(usecase::git_workflow::DiagnosticText::new(
                    "resolver-proven repository root does not match the runner repository",
                )))
            }
        }

        struct DispatchRunner;

        impl ReviewFixRunner for DispatchRunner {
            fn run_fix(
                &self,
                _command: RunReviewFixCommand,
            ) -> Result<RunReviewFixOutput, ReviewFixRunnerError> {
                Err(ReviewFixRunnerError::SubagentDispatchRequired(Box::new(
                    SubagentDispatchInstruction {
                        agent: SubagentName::try_new("review-fix-lead".to_owned())
                            .expect("valid test subagent"),
                        model: ModelName::try_new("gpt-5.5").expect("valid test model"),
                        effort: ReasoningEffort::Low,
                        scope: ReviewScopeName::try_new("cli_driver".to_owned())
                            .expect("valid test review scope"),
                        briefing_file: PathBuf::from("tmp/reviewer-runtime/briefing.md"),
                        track_id: ReviewTrackId::try_new("review-fix-driver-2026".to_owned())
                            .expect("valid test track ID"),
                        repository_root: PathBuf::from("/resolver-proven/root"),
                        round_type: ReviewRoundType::Fast,
                    },
                )))
            }
        }

        let input = || {
            ReviewFixInput::new(
                "cli_driver".to_owned(),
                PathBuf::from("tmp/reviewer-runtime/briefing.md"),
                None,
                PathBuf::from("track/items"),
                "fast".to_owned(),
                None,
            )
        };
        let runner_failure = ReviewFixDriver::new(Arc::new(RunReviewFixInteractor::new(
            Arc::new(ResolvingTrack),
            Arc::new(SpawnFailingRunner),
        )))
        .handle(input());
        let resolution_failure = ReviewFixDriver::new(Arc::new(RunReviewFixInteractor::new(
            Arc::new(NonTrackBranch),
            Arc::new(SpawnFailingRunner),
        )))
        .handle(input());
        let dispatch = ReviewFixDriver::new(Arc::new(RunReviewFixInteractor::new(
            Arc::new(ResolvingTrack),
            Arc::new(DispatchRunner),
        )))
        .handle(input());
        let root_mismatch = ReviewFixDriver::new(Arc::new(RunReviewFixInteractor::new(
            Arc::new(ResolvingTrack),
            Arc::new(RootMismatchRunner),
        )))
        .handle(input());

        assert_ne!(runner_failure.exit_code, 0);
        assert!(runner_failure.stderr.as_deref().is_some_and(|message| {
            message.contains("fix runner failed: spawn failed: runner unavailable")
        }));
        assert_ne!(resolution_failure.exit_code, 0);
        assert!(
            resolution_failure
                .stderr
                .as_deref()
                .is_some_and(|message| message.contains("track resolution failed"))
        );
        assert_eq!(dispatch.exit_code, SUBAGENT_DISPATCH_EXIT_CODE);
        assert!(
            dispatch
                .stdout
                .as_deref()
                .is_some_and(|output| output.starts_with(SUBAGENT_DISPATCH_SENTINEL))
        );
        assert_ne!(root_mismatch.exit_code, 0);
        assert!(root_mismatch.stderr.as_deref().is_some_and(|message| {
            message.contains("resolver-proven repository root does not match the runner repository")
        }));
    }

    #[test]
    fn test_review_fix_driver_renders_round_smoke_and_track_mismatch_errors() {
        let service = Arc::new(CapturingFixService { received: Mutex::new(false) });
        let invalid_round_type = ReviewFixDriver::new(service.clone()).handle(ReviewFixInput::new(
            "cli_driver".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            None,
            PathBuf::from("track/items"),
            "later".to_owned(),
            None,
        ));

        assert_ne!(invalid_round_type.exit_code, 0);
        assert!(
            invalid_round_type
                .stderr
                .as_deref()
                .is_some_and(|message| message.contains("invalid review-fix round type"))
        );
        assert!(
            !*service.received.lock().expect("test mutex must not be poisoned"),
            "invalid round types must not invoke the injected service"
        );

        struct ResolvingTrack;

        impl ReviewFixTrackResolverPort for ResolvingTrack {
            fn resolve_current_track(
                &self,
                _items_dir: &Path,
            ) -> Result<ReviewFixResolution, ReviewFixTrackResolveError> {
                Ok(ReviewFixResolution::new(
                    ReviewTrackId::try_new("review-fix-driver-2026".to_owned())
                        .expect("fixed test track ID must be valid"),
                    PathBuf::from("/test-repository"),
                ))
            }
        }

        struct SmokeFailingRunner;

        impl ReviewFixRunner for SmokeFailingRunner {
            fn run_fix(
                &self,
                _command: RunReviewFixCommand,
            ) -> Result<RunReviewFixOutput, ReviewFixRunnerError> {
                Err(ReviewFixRunnerError::SmokeTestFailed(
                    usecase::git_workflow::DiagnosticText::new("sandbox denied"),
                ))
            }
        }

        let input = |track_id| {
            ReviewFixInput::new(
                "cli_driver".to_owned(),
                PathBuf::from("tmp/reviewer-runtime/briefing.md"),
                track_id,
                PathBuf::from("track/items"),
                "fast".to_owned(),
                None,
            )
        };
        let smoke_test_failure = ReviewFixDriver::new(Arc::new(RunReviewFixInteractor::new(
            Arc::new(ResolvingTrack),
            Arc::new(SmokeFailingRunner),
        )))
        .handle(input(None));
        let track_mismatch = ReviewFixDriver::new(Arc::new(RunReviewFixInteractor::new(
            Arc::new(ResolvingTrack),
            Arc::new(SmokeFailingRunner),
        )))
        .handle(input(Some("other-track-2026".to_owned())));

        assert_eq!(smoke_test_failure.exit_code, 2);
        assert_eq!(smoke_test_failure.stdout, None);
        assert_eq!(
            smoke_test_failure.stderr.as_deref(),
            Some("[ERROR] smoke test failed: sandbox denied")
        );
        assert_ne!(track_mismatch.exit_code, 0);
        assert!(
            track_mismatch.stderr.as_deref().is_some_and(|message| message.contains(
                "explicit track 'other-track-2026' does not match current branch track 'review-fix-driver-2026'"
            ))
        );
    }

    #[test]
    fn test_review_fix_driver_renders_all_runner_error_variants() {
        struct RunnerErrorService {
            error: Mutex<Option<ReviewFixRunnerError>>,
        }

        impl RunReviewFixService for RunnerErrorService {
            fn run(
                &self,
                _request: RunReviewFixRequest,
            ) -> Result<RunReviewFixOutput, RunReviewFixError> {
                let error = self
                    .error
                    .lock()
                    .expect("test mutex must not be poisoned")
                    .take()
                    .expect("runner error service must be invoked once");
                Err(RunReviewFixError::FixRunnerFailed(error))
            }
        }

        let input = || {
            ReviewFixInput::new(
                "cli_driver".to_owned(),
                PathBuf::from("tmp/reviewer-runtime/briefing.md"),
                None,
                PathBuf::from("track/items"),
                "fast".to_owned(),
                None,
            )
        };
        let render = |error: ReviewFixRunnerError| {
            ReviewFixDriver::new(Arc::new(RunnerErrorService { error: Mutex::new(Some(error)) }))
                .handle(input())
        };

        let smoke = render(ReviewFixRunnerError::SmokeTestFailed(DiagnosticText::new("sandbox")));
        let spawn = render(ReviewFixRunnerError::SpawnFailed(DiagnosticText::new("spawn")));
        let sentinel =
            render(ReviewFixRunnerError::SentinelNotFound(DiagnosticText::new("sentinel")));
        let dispatch = render(ReviewFixRunnerError::SubagentDispatchRequired(Box::new(
            SubagentDispatchInstruction {
                agent: SubagentName::try_new("review-fix-lead".to_owned())
                    .expect("valid test subagent"),
                model: ModelName::try_new("gpt-5.5").expect("valid test model"),
                effort: ReasoningEffort::Low,
                scope: ReviewScopeName::try_new("cli_driver".to_owned())
                    .expect("valid test review scope"),
                briefing_file: PathBuf::from("tmp/reviewer-runtime/briefing.md"),
                track_id: ReviewTrackId::try_new("review-fix-driver-2026".to_owned())
                    .expect("valid test track ID"),
                repository_root: PathBuf::from("/resolver-proven/root"),
                round_type: ReviewRoundType::Fast,
            },
        )));
        let unexpected =
            render(ReviewFixRunnerError::Unexpected(DiagnosticText::new("unexpected")));

        assert_eq!(smoke.exit_code, 2);
        assert_eq!(smoke.stdout, None);
        assert_eq!(smoke.stderr.as_deref(), Some("[ERROR] smoke test failed: sandbox"));
        assert_ne!(spawn.exit_code, 0);
        assert!(spawn.stderr.as_deref().is_some_and(|message| message.contains("spawn failed")));
        assert_ne!(sentinel.exit_code, 0);
        assert!(
            sentinel
                .stderr
                .as_deref()
                .is_some_and(|message| message.contains("sentinel not found in output"))
        );
        assert_eq!(dispatch.exit_code, SUBAGENT_DISPATCH_EXIT_CODE);
        assert!(
            dispatch
                .stdout
                .as_deref()
                .is_some_and(|output| output.starts_with(SUBAGENT_DISPATCH_SENTINEL))
        );
        assert_ne!(unexpected.exit_code, 0);
        assert!(
            unexpected
                .stderr
                .as_deref()
                .is_some_and(|message| message.contains("unexpected error"))
        );
    }

    #[test]
    fn test_subagent_dispatch_instruction_renders_sentinel_and_single_line_json() {
        let outcome = subagent_dispatch_to_outcome(SubagentDispatchInstruction {
            agent: SubagentName::try_new("review-fix-lead".to_owned())
                .expect("valid test subagent"),
            model: ModelName::try_new("claude\"model").expect("valid test model"),
            effort: ReasoningEffort::Low,
            scope: ReviewScopeName::try_new("cli_driver".to_owned())
                .expect("valid test review scope"),
            briefing_file: PathBuf::from("tmp/reviewer-runtime/briefing\\path.md"),
            track_id: ReviewTrackId::try_new("dispatch-render-2026".to_owned())
                .expect("valid test track ID"),
            repository_root: PathBuf::from("/resolver-proven/root\\path"),
            round_type: ReviewRoundType::Fast,
        });

        assert_eq!(outcome.exit_code, SUBAGENT_DISPATCH_EXIT_CODE);
        let stdout = outcome.stdout.unwrap();
        let mut lines = stdout.lines();
        assert_eq!(lines.next(), Some(SUBAGENT_DISPATCH_SENTINEL));
        assert_eq!(
            lines.next(),
            Some(
                "{\"agent\":\"review-fix-lead\",\"model\":\"claude\\\"model\",\"effort\":\"low\",\"scope\":\"cli_driver\",\"briefing_file\":\"tmp/reviewer-runtime/briefing\\\\path.md\",\"track_id\":\"dispatch-render-2026\",\"repository_root\":\"/resolver-proven/root\\\\path\",\"round_type\":\"fast\"}"
            )
        );
        assert_eq!(lines.next(), None, "dispatch JSON must occupy one line");
    }

    #[cfg(unix)]
    #[test]
    fn test_subagent_dispatch_instruction_rejects_non_utf8_briefing_path() {
        let outcome = subagent_dispatch_to_outcome(SubagentDispatchInstruction {
            agent: SubagentName::try_new("review-fix-lead".to_owned())
                .expect("valid test subagent"),
            model: ModelName::try_new("gpt-5.5".to_owned()).expect("valid test model"),
            effort: ReasoningEffort::Low,
            scope: ReviewScopeName::try_new("cli_driver".to_owned())
                .expect("valid test review scope"),
            briefing_file: PathBuf::from(OsString::from_vec(vec![b'b', 0x80])),
            track_id: ReviewTrackId::try_new("dispatch-render-2026".to_owned())
                .expect("valid test track ID"),
            repository_root: PathBuf::from("/resolver-proven/root"),
            round_type: ReviewRoundType::Fast,
        });

        assert_ne!(outcome.exit_code, SUBAGENT_DISPATCH_EXIT_CODE);
        assert_eq!(outcome.stdout, None);
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|message| { message.contains("briefing path is not valid UTF-8") })
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_subagent_dispatch_instruction_rejects_non_utf8_repository_root() {
        let outcome = subagent_dispatch_to_outcome(SubagentDispatchInstruction {
            agent: SubagentName::try_new("review-fix-lead".to_owned())
                .expect("valid test subagent"),
            model: ModelName::try_new("gpt-5.5".to_owned()).expect("valid test model"),
            effort: ReasoningEffort::Low,
            scope: ReviewScopeName::try_new("cli_driver".to_owned())
                .expect("valid test review scope"),
            briefing_file: PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            track_id: ReviewTrackId::try_new("dispatch-render-2026".to_owned())
                .expect("valid test track ID"),
            repository_root: PathBuf::from(OsString::from_vec(vec![b'/', 0x80])),
            round_type: ReviewRoundType::Fast,
        });

        assert_ne!(outcome.exit_code, SUBAGENT_DISPATCH_EXIT_CODE);
        assert_eq!(outcome.stdout, None);
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|message| { message.contains("repository root is not valid UTF-8") })
        );
    }
}
