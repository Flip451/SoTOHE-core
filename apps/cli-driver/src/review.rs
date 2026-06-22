// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `review` command family — primary adapter driver.
//!
//! `ReviewDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The render helpers here mirror those in
//! `apps/cli-composition/src/review_v2/results.rs` (lines 24-244
//! `render_review_results_str` — the ~220-line rendering engine) and
//! `apps/cli-composition/src/review_v2/mod.rs` (lines 465-491
//! `review_check_approved` message assembly);
//! T021 removes the `cli_composition` duplicates when the live path is flipped.

// TODO(T021): add use-case + infrastructure imports once Cargo.toml is materialized.
// use std::path::{Path, PathBuf};
// use std::collections::HashMap;
// use std::fmt::Write as _;
// use std::time::Duration;
// use domain::TrackId;
// use domain::review_v2::{
//     NotRequiredReason, ReviewApprovalVerdict, ReviewApprovalDecision,
//     ReviewExistsPort as _, ReviewReader, ReviewState,
//     ReviewerFinding, RoundType, ScopeName, ScopeRound, Verdict,
// };
// use infrastructure::review_v2::{ClaudeReviewer, CodexReviewer};
// use usecase::review_v2::ReviewApprovalDecision;

use std::path::PathBuf;

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
        /// Track ID (auto-detected from branch if `None`).
        track_id: Option<String>,
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
        /// Track ID (auto-detected from branch if `None`).
        track_id: Option<String>,
        /// Items directory (`track/items`).
        items_dir: PathBuf,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `review` command family.
///
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct ReviewDriver {
    // TODO(T021): inject use-case interactors here (currently this family has
    // no injectable adapter dependencies — infrastructure functions are called
    // inline, same as cli_composition::ReviewCompositionRoot).
}

impl ReviewDriver {
    /// Create a new `ReviewDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a review command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
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
            ReviewInput::PersistCommitHash { track_id, items_dir } => {
                self.review_persist_commit_hash(track_id, items_dir)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Render helpers (logic duplicated from cli_composition/src/review_v2/mod.rs
    // and review_v2/results.rs; T021 removes the cli_composition copies).
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn review_run_codex(
        &self,
        _model: String,
        _timeout_seconds: u64,
        _briefing_file: Option<PathBuf>,
        _prompt: Option<String>,
        _track_id: Option<String>,
        _round_type: String,
        _group: String,
        _items_dir: PathBuf,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id for write, validate group name, build CodexReviewer,
        // then invoke run_codex_review_str and outcome_to_command_outcome.
        // Emit ReviewRound + ExternalSubprocess telemetry.
        // Mirrors cli_composition/src/review_v2/mod.rs ReviewCompositionRoot::review_run_codex.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    #[allow(clippy::too_many_arguments)]
    fn review_run_claude(
        &self,
        _model: String,
        _timeout_seconds: u64,
        _briefing_file: Option<PathBuf>,
        _prompt: Option<String>,
        _track_id: Option<String>,
        _round_type: String,
        _group: String,
        _items_dir: PathBuf,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id for write, validate group name, build ClaudeReviewer,
        // then invoke run_claude_review_str and outcome_to_command_outcome.
        // Emit ReviewRound + ExternalSubprocess telemetry.
        // Mirrors cli_composition/src/review_v2/mod.rs ReviewCompositionRoot::review_run_claude.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    #[allow(clippy::too_many_arguments)]
    fn review_run_local(
        &self,
        _model: Option<String>,
        _timeout_seconds: u64,
        _briefing_file: Option<PathBuf>,
        _prompt: Option<String>,
        _track_id: Option<String>,
        _round_type: String,
        _group: String,
        _items_dir: PathBuf,
    ) -> CommandOutcome {
        // TODO(T021): resolve reviewer capability from agent-profiles.json, then dispatch to
        // review_run_codex or review_run_claude based on resolved provider.
        // Mirrors cli_composition/src/review_v2/mod.rs ReviewCompositionRoot::review_run_local.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn review_run_fix_local(
        &self,
        _scope: String,
        _briefing_file: PathBuf,
        _track_id: String,
        _round_type: String,
        _model: Option<String>,
    ) -> CommandOutcome {
        // TODO(T021): build RunReviewFixLocalInput, invoke run_fix_local via
        // infrastructure::review_v2 fix-runner adapter.
        // Mirrors cli_composition/src/review_v2/mod.rs ReviewCompositionRoot::review_run_fix_local.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn review_check_approved(
        &self,
        _track_id: Option<String>,
        _items_dir: PathBuf,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id from branch if None, invoke check_approved_str, then
        // build message from ReviewApprovalDecision:
        //   Approved            → "[OK] Review is approved and code hash is current"  (exit 0)
        //   ApprovedWithBypass  → format!("[WARN] No review.json found. Allowing commit for \
        //                           PR-based review ({count} scope(s)).")  (exit 0)
        //   Blocked             → format!("[BLOCKED] Review not approved. Required scopes:\n{}", …)
        //                         (exit 1)
        // Mirrors cli_composition/src/review_v2/mod.rs ReviewCompositionRoot::review_check_approved
        // (lines 465-491).
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    #[allow(clippy::too_many_arguments)]
    fn review_results(
        &self,
        _track_id: Option<String>,
        _items_dir: PathBuf,
        _scope: Option<String>,
        _all: bool,
        _limit: u32,
        _round_type: String,
        _no_hint: bool,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id from branch if None; convert limit: u32 == 0 → None,
        // then invoke render_review_results_str(&track_id, &items_dir, scope.as_deref(),
        //   limit_opt, &round_type, no_hint) and return CommandOutcome::success(Some(output)).
        // Mirrors cli_composition/src/review_v2/mod.rs ReviewCompositionRoot::review_results.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn review_classify(
        &self,
        _paths: Vec<String>,
        _track_id: Option<String>,
        _items_dir: PathBuf,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id from branch if None, validate all paths,
        // build scope query interactor (no-diff variant), classify via ScopeQueryService::classify_by_strings,
        // then format each entry as "{path}\t{scopes_csv}\n".
        // Mirrors cli_composition/src/review_v2/mod.rs ReviewCompositionRoot::review_classify.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn review_files(
        &self,
        _scope: String,
        _track_id: Option<String>,
        _items_dir: PathBuf,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id from branch if None, validate scope name,
        // build scope query interactor (with diff), call ScopeQueryService::files_by_string,
        // then format each file as "{file}\n".
        // Mirrors cli_composition/src/review_v2/mod.rs ReviewCompositionRoot::review_files.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn review_validate_scope(
        &self,
        _scope: String,
        _track_id: Option<String>,
        _items_dir: PathBuf,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id from branch if None, validate scope name via
        // validate_scope_for_track_str, return CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned())) on success.
        // Mirrors cli_composition/src/review_v2/mod.rs ReviewCompositionRoot::review_validate_scope.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn review_get_briefing(
        &self,
        _scope: String,
        _track_id: Option<String>,
        _items_dir: PathBuf,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id from branch if None, call get_briefing_for_scope_str,
        // return CommandOutcome::success(maybe_path) where maybe_path: Option<String>.
        // Mirrors cli_composition/src/review_v2/mod.rs ReviewCompositionRoot::review_get_briefing.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn review_persist_commit_hash(
        &self,
        _track_id: Option<String>,
        _items_dir: PathBuf,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id from branch if None, invoke
        // persist_commit_hash_for_track(&track_id), emit eprintln("[review] Recorded .commit_hash: …"),
        // return CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned())).
        // Mirrors cli_composition/src/review_v2/mod.rs ReviewCompositionRoot::review_persist_commit_hash.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }
}

impl Default for ReviewDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Render helpers (duplicated from cli_composition/src/review_v2/results.rs
// lines 24-244 and mod.rs lines 465-491;
// T021 removes the cli_composition copies and moves these to cli_driver::render).
// ---------------------------------------------------------------------------

/// Renders the `sotp review results` output as a string, given string-typed parameters.
///
/// Mirrors `cli_composition::review_v2::results::render_review_results_str`
/// (results.rs lines 24-244 — the ~220-line rendering engine).
///
/// TODO(T021): wire real domain types (`domain::TrackId`, `domain::review_v2::*`,
/// `infrastructure::review_v2::*`) once the dependency graph is materialized.
/// Currently returns a placeholder so the staged file is self-consistent.
///
/// # Errors
/// Returns a human-readable error string on any I/O or domain failure.
#[allow(dead_code)]
fn render_review_results_str(
    _track_id_str: &str,
    _items_dir: &std::path::Path,
    _scope_filter: Option<&str>,
    _limit: Option<u32>,
    _round_type: &str,
    _no_hint: bool,
) -> Result<String, String> {
    // TODO(T021): Paste full implementation from
    // cli_composition/src/review_v2/results.rs lines 32-243 once `domain` and
    // `infrastructure` crates are available as dependencies. The implementation:
    //
    //   1. TrackId::try_new(track_id_str) → validate track id
    //   2. build_review_v2(&track_id, items_dir) → ReviewV2Composition { cycle, review_store, base }
    //   3. cycle.get_review_states(&review_store) → HashMap<ScopeName, ReviewState>
    //   4. review_store.review_json_exists() → bool
    //   5. cycle.evaluate_approval(&review_store, review_json_exists) → ReviewApprovalVerdict
    //   6. Sort scope_universe alphabetically; optionally filter by scope_filter
    //   7. Load all rounds per scope via review_store.read_all_rounds(scope)
    //   8. Render per-scope state lines with indicator ([+]/[-]/[.]) and
    //      round-type@timestamp suffix from state_line_suffix(rounds)
    //   9. For displayed rounds (filtered by limit + round_type): render_findings_block
    //  10. Append summary: "Summary: {approved} approved, {empty} empty, {required} required, {total} total"
    //  11. If Approved + review_json_exists + !no_hint: emit commit hint
    Ok(String::new())
}

/// Format the `review check-approved` stderr message from decision fields.
///
/// Mirrors the inline message assembly in
/// `cli_composition::review_v2::mod::ReviewCompositionRoot::review_check_approved`
/// (mod.rs lines 465-491).
///
/// TODO(T021): call with real `usecase::review_v2::ReviewApprovalDecision` once the
/// dependency graph is materialized.
#[allow(dead_code)]
fn format_check_approved_msg(
    is_approved: bool,
    is_approved_with_bypass: bool,
    bypass_scope_count: usize,
    blocked_scopes: &[&str],
) -> (String, u8) {
    if is_approved {
        ("[OK] Review is approved and code hash is current".to_owned(), 0u8)
    } else if is_approved_with_bypass {
        (
            format!(
                "[WARN] No review.json found. Allowing commit for PR-based review \
                 ({bypass_scope_count} scope(s))."
            ),
            0u8,
        )
    } else {
        let mut display: Vec<String> =
            blocked_scopes.iter().map(|scope| format!("  {scope}")).collect();
        display.sort();
        (format!("[BLOCKED] Review not approved. Required scopes:\n{}", display.join("\n")), 1u8)
    }
}
