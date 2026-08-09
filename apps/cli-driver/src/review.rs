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
    RunReviewFixCommand, RunReviewFixError, RunReviewFixOutput, RunReviewFixService,
};
use usecase::review_v2::{
    ReviewCheckZeroFindingsError, ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsQuery,
};
use usecase::review_v2::{ReviewCheckZeroFindingsService, ReviewService};

use crate::render::CommandOutcome;

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
    result: Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsError>,
) -> CommandOutcome {
    match result {
        Ok(outcome) => check_zero_findings_outcome_to_command_outcome(outcome),
        Err(error) => CommandOutcome::failure(Some(error.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `review` command family.
pub enum ReviewInput {
    /// Run the local Codex-backed reviewer and auto-record verdict to review.json.
    RunCodex {
        /// Model name for the Codex reviewer subprocess.
        model: String,
        /// Timeout for the reviewer subprocess in seconds.
        timeout_seconds: u64,
        /// Optional path to a briefing file for additional context.
        briefing_file: Option<PathBuf>,
        /// Optional inline prompt override.
        prompt: Option<String>,
        /// Track ID (auto-detected from branch if `None`).
        track_id: Option<String>,
        /// Round type: `"fast"` or `"final"`.
        round_type: String,
        /// Scope name (e.g., `"cli"`, `"infrastructure"`).
        group: String,
        /// Items directory (`track/items`).
        items_dir: PathBuf,
    },
    /// Run the Claude-backed reviewer and auto-record verdict to review.json.
    RunClaude {
        /// Model name for the Claude reviewer subprocess.
        model: String,
        /// Timeout for the reviewer subprocess in seconds.
        timeout_seconds: u64,
        /// Optional path to a briefing file for additional context.
        briefing_file: Option<PathBuf>,
        /// Optional inline prompt override.
        prompt: Option<String>,
        /// Track ID (auto-detected from branch if `None`).
        track_id: Option<String>,
        /// Round type: `"fast"` or `"final"`.
        round_type: String,
        /// Scope name (e.g., `"cli"`, `"infrastructure"`).
        group: String,
        /// Items directory (`track/items`).
        items_dir: PathBuf,
    },
    /// Run the auto-dispatched local reviewer (provider resolved from agent-profiles.json).
    RunLocal {
        /// Optional model override (uses profile model when `None`).
        model: Option<String>,
        /// Timeout for the reviewer subprocess in seconds.
        timeout_seconds: u64,
        /// Optional path to a briefing file for additional context.
        briefing_file: Option<PathBuf>,
        /// Optional inline prompt override.
        prompt: Option<String>,
        /// Track ID (auto-detected from branch if `None`).
        track_id: Option<String>,
        /// Round type: `"fast"` or `"final"`.
        round_type: String,
        /// Scope name (e.g., `"cli"`, `"infrastructure"`).
        group: String,
        /// Items directory (`track/items`).
        items_dir: PathBuf,
    },
    /// Check if the review state is approved and code hash is current.
    CheckApproved {
        /// Resolved track ID.
        track_id: String,
        /// Items directory (`track/items`).
        items_dir: PathBuf,
    },
    /// Check whether one resolved track and scope have a current final
    /// zero-findings review verdict.
    CheckZeroFindings(ReviewCheckZeroFindingsQuery),
    /// Show review results: per-scope state summary, optional round history.
    Results {
        /// Track ID (auto-detected from branch if `None`).
        track_id: Option<String>,
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Optional scope name filter.
        scope: Option<String>,
        /// Show all rounds (equivalent to `--limit 0` when `false`).
        all: bool,
        /// Maximum number of rounds to display per scope; `0` = summary only.
        limit: u32,
        /// Round type filter: `"any"` | `"fast"` | `"final"`.
        round_type: String,
        /// Suppress the commit hint line.
        no_hint: bool,
    },
    /// Classify each given path into review scopes.
    Classify {
        /// Paths to classify.
        paths: Vec<String>,
        /// Track ID (auto-detected from branch if `None`).
        track_id: Option<String>,
        /// Items directory (`track/items`).
        items_dir: PathBuf,
    },
    /// List the diff files belonging to the given scope.
    Files {
        /// Scope name.
        scope: String,
        /// Track ID (auto-detected from branch if `None`).
        track_id: Option<String>,
        /// Items directory (`track/items`).
        items_dir: PathBuf,
    },
    /// Validate a scope name for the given track.
    ValidateScope {
        /// Scope name to validate.
        scope: String,
        /// Track ID (auto-detected from branch if `None`).
        track_id: Option<String>,
        /// Items directory (`track/items`).
        items_dir: PathBuf,
    },
    /// Get the briefing for a review scope.
    GetBriefing {
        /// Scope name.
        scope: String,
        /// Track ID (auto-detected from branch if `None`).
        track_id: Option<String>,
        /// Items directory (`track/items`).
        items_dir: PathBuf,
    },
    /// Persist a commit hash for the review cycle.
    PersistCommitHash {
        /// Resolved track ID.
        track_id: String,
        /// Workspace root (the repo root where `.git` lives).
        workspace_root: PathBuf,
    },
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
    check_zero_findings: Arc<dyn ReviewCheckZeroFindingsService>,
}

impl ReviewDriver {
    /// Create a new `ReviewDriver` with its aggregate and focused services.
    pub fn new(
        service: Arc<dyn ReviewService>,
        check_zero_findings: Arc<dyn ReviewCheckZeroFindingsService>,
    ) -> Self {
        Self { service, check_zero_findings }
    }

    /// Handle a review command.
    pub fn handle(&self, input: ReviewInput) -> CommandOutcome {
        match input {
            ReviewInput::RunCodex {
                model,
                timeout_seconds,
                briefing_file,
                prompt,
                track_id,
                round_type,
                group,
                items_dir,
            } => self.review_run_codex(
                model,
                timeout_seconds,
                briefing_file,
                prompt,
                track_id,
                round_type,
                group,
                items_dir,
            ),
            ReviewInput::RunClaude {
                model,
                timeout_seconds,
                briefing_file,
                prompt,
                track_id,
                round_type,
                group,
                items_dir,
            } => self.review_run_claude(
                model,
                timeout_seconds,
                briefing_file,
                prompt,
                track_id,
                round_type,
                group,
                items_dir,
            ),
            ReviewInput::RunLocal {
                model,
                timeout_seconds,
                briefing_file,
                prompt,
                track_id,
                round_type,
                group,
                items_dir,
            } => self.review_run_local(
                model,
                timeout_seconds,
                briefing_file,
                prompt,
                track_id,
                round_type,
                group,
                items_dir,
            ),
            ReviewInput::CheckApproved { track_id, items_dir } => {
                self.review_check_approved(track_id, items_dir)
            }
            ReviewInput::CheckZeroFindings(query) => self.review_check_zero_findings(query),
            ReviewInput::Results {
                track_id,
                items_dir,
                scope,
                all,
                limit,
                round_type,
                no_hint,
            } => self.review_results(track_id, items_dir, scope, all, limit, round_type, no_hint),
            ReviewInput::Classify { paths, track_id, items_dir } => {
                self.review_classify(paths, track_id, items_dir)
            }
            ReviewInput::Files { scope, track_id, items_dir } => {
                self.review_files(scope, track_id, items_dir)
            }
            ReviewInput::ValidateScope { scope, track_id, items_dir } => {
                self.review_validate_scope(scope, track_id, items_dir)
            }
            ReviewInput::GetBriefing { scope, track_id, items_dir } => {
                self.review_get_briefing(scope, track_id, items_dir)
            }
            ReviewInput::PersistCommitHash { track_id, workspace_root } => {
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
        CommandOutcome { stdout: out.stdout, stderr: out.stderr, exit_code: out.exit_code }
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

    #[allow(clippy::too_many_arguments)]
    fn review_results(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
        scope: Option<String>,
        all: bool,
        limit: u32,
        round_type: String,
        no_hint: bool,
    ) -> CommandOutcome {
        match self.service.results(track_id, items_dir, scope, all, limit, round_type, no_hint) {
            Ok(output) => CommandOutcome::success(Some(output)),
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
            Ok(sha) => {
                eprintln!("[review] Recorded .commit_hash: {sha}");
                CommandOutcome::success(None)
            }
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }
}

/// Driving adapter for one fully-wired review-fix invocation.
///
/// Composition supplies the validated command and an injected
/// [`RunReviewFixService`]. This driver owns the interactor invocation and all
/// review-fix stdout/stderr and exit-code rendering.
pub struct ReviewFixDriver {
    service: Arc<dyn RunReviewFixService>,
    command: RunReviewFixCommand,
    provider: String,
}

impl ReviewFixDriver {
    /// Creates a review-fix driver from composition-owned wiring.
    #[must_use]
    pub fn new(
        service: Arc<dyn RunReviewFixService>,
        command: RunReviewFixCommand,
        provider: String,
    ) -> Self {
        Self { service, command, provider }
    }

    /// Executes the injected interactor and renders the review-fix protocol.
    #[must_use]
    pub fn handle(&self) -> CommandOutcome {
        eprintln!(
            "[sotp review fix-local] provider={} model={}",
            self.provider, self.command.model
        );
        match self.service.run(self.command.clone()) {
            Ok(output) => review_fix_output_to_outcome(output),
            Err(RunReviewFixError::SubagentDispatchRequired(instruction)) => {
                subagent_dispatch_to_outcome(*instruction)
            }
            // SmokeTestFailed is a preflight failure (not a review outcome).
            // Preserve exit 2 + diagnostic on stderr without emitting a
            // `REVIEW_FIX_STATUS:` line so orchestrators do not classify it
            // as a normal review-fix outcome.
            Err(RunReviewFixError::SmokeTestFailed(message)) => CommandOutcome {
                stdout: None,
                stderr: Some(format!("[ERROR] smoke test failed: {}", message.as_str())),
                exit_code: 2,
            },
            Err(error) => CommandOutcome::failure(Some(error.to_string())),
        }
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
        return CommandOutcome::success(Some(
            r#"{"verdict":"zero_findings","findings":[]}"#.to_owned(),
        ));
    }
    // Preserve the underlying reviewer's exit code so the convention that
    // `findings_remain` returns exit 2 (distinguishing review findings from
    // subprocess failures) survives the cli_driver boundary.
    CommandOutcome { stdout: out.summary, stderr: None, exit_code: out.exit_code }
}

fn subagent_dispatch_to_outcome(instruction: SubagentDispatchInstruction) -> CommandOutcome {
    let json = format!(
        "{{\"agent\":{},\"model\":{},\"effort\":{},\"scope\":{},\"briefing_file\":{},\"track_id\":{},\"round_type\":{}}}",
        json_str(instruction.agent.as_str()),
        json_str(instruction.model.as_str()),
        json_str(effort_value(instruction.effort)),
        json_str(instruction.scope.as_ref()),
        json_str(&instruction.briefing_file.display().to_string()),
        json_str(instruction.track_id.as_ref()),
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
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use domain::review_v2::{MainScopeName, ScopeName};
    use usecase::capability_exec::{ModelName, ReasoningEffort};
    use usecase::review_v2::run_review_fix::{
        RunReviewFixCommand, RunReviewFixError, RunReviewFixOutput, RunReviewFixService,
    };
    use usecase::review_v2::{
        ReviewCheckZeroFindingsError, ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsQuery,
        ReviewCheckZeroFindingsService, ReviewGroupName, ReviewRoundType, ReviewService,
        SubagentDispatchInstruction, SubagentName, TrackId,
    };

    use super::{
        ReviewDriver, ReviewFixDriver, ReviewInput, SUBAGENT_DISPATCH_EXIT_CODE,
        SUBAGENT_DISPATCH_SENTINEL, check_zero_findings_outcome_to_command_outcome,
        check_zero_findings_result_to_command_outcome, subagent_dispatch_to_outcome,
    };

    struct UnusedReviewService;

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

        fn results(
            &self,
            _track_id: Option<String>,
            _items_dir: PathBuf,
            _scope: Option<String>,
            _all: bool,
            _limit: u32,
            _round_type: String,
            _no_hint: bool,
        ) -> Result<String, usecase::review_v2::ReviewAuxError> {
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

    struct CapturingCheckZeroFindingsService {
        received_query: Mutex<Option<ReviewCheckZeroFindingsQuery>>,
        result: Mutex<Option<Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsError>>>,
    }

    impl CapturingCheckZeroFindingsService {
        fn new(
            result: Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsError>,
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
        ) -> Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsError> {
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
        ReviewCheckZeroFindingsQuery {
            track: TrackId::try_new("review-driver-check-2026").expect("valid test track ID"),
            items_dir: PathBuf::from("track/items"),
            scope: ScopeName::Main(MainScopeName::new("cli_driver").expect("valid test scope")),
        }
    }

    fn review_driver_for_check(
        result: Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsError>,
    ) -> (ReviewDriver, Arc<CapturingCheckZeroFindingsService>) {
        let focused_service = Arc::new(CapturingCheckZeroFindingsService::new(result));
        let driver = ReviewDriver::new(Arc::new(UnusedReviewService), focused_service.clone());
        (driver, focused_service)
    }

    #[test]
    fn test_review_driver_check_zero_findings_dispatches_success_to_focused_service() {
        let query = check_zero_findings_query();
        let (driver, focused_service) =
            review_driver_for_check(Ok(ReviewCheckZeroFindingsOutcome::CurrentFinalZeroFindings));

        let outcome = driver.handle(ReviewInput::CheckZeroFindings(query.clone()));

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
            let (driver, focused_service) = review_driver_for_check(result);

            let outcome = driver.handle(ReviewInput::CheckZeroFindings(query.clone()));

            assert_eq!(focused_service.received_query(), Some(query));
            assert_ne!(outcome.exit_code, 0);
            assert!(outcome.stderr.is_some());
        }
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
            ReviewCheckZeroFindingsError::EvaluationFailed(domain::FreeText::new(
                "review.json is malformed",
            )),
        ));

        assert_ne!(outcome.exit_code, 0);
        assert!(outcome.stderr.as_deref().is_some_and(|message| message.contains("malformed")));
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
            _command: RunReviewFixCommand,
        ) -> Result<RunReviewFixOutput, RunReviewFixError> {
            Ok(RunReviewFixOutput { status: "completed".to_owned(), exit_code: 0, stderr: None })
        }
    }

    #[test]
    fn test_review_fix_driver_completed_renders_status() {
        let driver = ReviewFixDriver::new(
            Arc::new(CompletedFixService),
            RunReviewFixCommand {
                scope: "cli_driver".to_owned(),
                briefing_file: PathBuf::from("tmp/reviewer-runtime/briefing.md"),
                track_id: "review-fix-driver-2026".to_owned(),
                round_type: "fast".to_owned(),
                model: "gpt-5.5".to_owned(),
            },
            "codex".to_owned(),
        );

        let outcome = driver.handle();

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("REVIEW_FIX_STATUS: completed"));
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_subagent_dispatch_instruction_renders_sentinel_and_single_line_json() {
        let outcome = subagent_dispatch_to_outcome(SubagentDispatchInstruction {
            agent: SubagentName::try_new("review-fix-lead").expect("valid test subagent"),
            model: ModelName::try_new("claude\"model").expect("valid test model"),
            effort: ReasoningEffort::Low,
            scope: ReviewGroupName::try_new("cli_driver").expect("valid test review group"),
            briefing_file: PathBuf::from("tmp/reviewer-runtime/briefing\\file.md"),
            track_id: TrackId::try_new("dispatch-render-2026").expect("valid test track ID"),
            round_type: ReviewRoundType::Fast,
        });

        assert_eq!(outcome.exit_code, SUBAGENT_DISPATCH_EXIT_CODE);
        let stdout = outcome.stdout.unwrap();
        let mut lines = stdout.lines();
        assert_eq!(lines.next(), Some(SUBAGENT_DISPATCH_SENTINEL));
        assert_eq!(
            lines.next(),
            Some(
                "{\"agent\":\"review-fix-lead\",\"model\":\"claude\\\"model\",\"effort\":\"low\",\"scope\":\"cli_driver\",\"briefing_file\":\"tmp/reviewer-runtime/briefing\\\\file.md\",\"track_id\":\"dispatch-render-2026\",\"round_type\":\"fast\"}"
            )
        );
        assert_eq!(lines.next(), None, "dispatch JSON must occupy one line");
    }
}
