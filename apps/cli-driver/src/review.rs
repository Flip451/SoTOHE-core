//! `review` command family — primary adapter driver.
//!
//! `ReviewDriver` holds a single injected `ReviewService` aggregate and exposes
//! `handle(input) -> CommandOutcome`. Each operation delegates to the
//! appropriate method on the service without importing `infrastructure` or
//! `domain`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::review_v2::ReviewService;
use usecase::review_v2::aggregate_service::{ReviewRunFixInput, ReviewRunInput};

use crate::render::CommandOutcome;

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
    /// Run the review-fix-lead fixer with provider auto-resolved from agent-profiles.json.
    RunFixLocal {
        /// Scope name (e.g., `"cli"`, `"infrastructure"`).
        scope: String,
        /// Path to the briefing file passed to the fixer. Required.
        briefing_file: PathBuf,
        /// Track ID. Required (no auto-resolve from branch for write operations).
        track_id: String,
        /// Round type: `"fast"` or `"final"`.
        round_type: String,
        /// Optional model override for the fixer subprocess.
        model: Option<String>,
    },
    /// Check if the review state is approved and code hash is current.
    CheckApproved {
        /// Resolved track ID.
        track_id: String,
        /// Items directory (`track/items`).
        items_dir: PathBuf,
    },
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
/// Holds a single injected `ReviewService` aggregate; exposes
/// `handle(input) -> CommandOutcome`. One injected interactor — no per-service
/// fields (D3/D4 cli_driver policy).
pub struct ReviewDriver {
    service: Arc<dyn ReviewService>,
}

impl ReviewDriver {
    /// Create a new `ReviewDriver` with a single injected aggregate service.
    pub fn new(service: Arc<dyn ReviewService>) -> Self {
        Self { service }
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
            ReviewInput::RunFixLocal { scope, briefing_file, track_id, round_type, model } => {
                self.review_run_fix_local(scope, briefing_file, track_id, round_type, model)
            }
            ReviewInput::CheckApproved { track_id, items_dir } => {
                self.review_check_approved(track_id, items_dir)
            }
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

    fn review_run_fix_local(
        &self,
        scope: String,
        briefing_file: PathBuf,
        track_id: String,
        round_type: String,
        model: Option<String>,
    ) -> CommandOutcome {
        let input = ReviewRunFixInput { scope, briefing_file, track_id, round_type, model };
        match self.service.run_fix_local(input) {
            Ok(out) => {
                // exit_code in RunReviewFixOutput is i32; map valid range to u8.
                let exit_code: u8 = match out.exit_code {
                    0 => 0,
                    2 => 2,
                    _ => 1,
                };
                CommandOutcome {
                    stdout: Some(format!("REVIEW_FIX_STATUS: {}", out.status)),
                    stderr: None,
                    exit_code,
                }
            }
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run_review_output_to_outcome(out: usecase::review_v2::RunReviewOutput) -> CommandOutcome {
    if out.skipped {
        return CommandOutcome::success(Some(
            r#"{"verdict":"zero_findings","findings":[]}"#.to_owned(),
        ));
    }
    match out.summary {
        Some(summary) => {
            let exit_code: u8 = if out.verdict_kind == "rejected" { 1 } else { 0 };
            CommandOutcome { stdout: Some(summary), stderr: None, exit_code }
        }
        None => {
            let exit_code: u8 = if out.verdict_kind == "rejected" { 1 } else { 0 };
            CommandOutcome { stdout: None, stderr: None, exit_code }
        }
    }
}
