//! `review_v2` command family — composition logic and CliApp impl methods.

pub(crate) mod approved;
pub(crate) mod briefing;
pub(crate) mod commit_hash;
mod helpers;
#[cfg(test)]
pub(crate) use helpers::process_guards;
pub(crate) use helpers::record_instant_once;
mod inputs;
pub(crate) mod null_reviewer;
pub(crate) mod results;
pub(crate) mod run;
pub mod run_fix;
pub(crate) mod scope;
pub(crate) mod shared;
mod shim;

pub use inputs::{
    ReviewResultsInput, ReviewRunClaudeInput, ReviewRunCodexInput, ReviewRunLocalInput,
    RunReviewFixLocalInput,
};

// Public re-exports: only items consumed by external crates (e.g. apps/cli).
// All composition builders, infrastructure-typed helpers, and internal DTOs are
// pub(crate) — they do not appear on the cli_composition public face (CN-02).
pub use briefing::append_scope_briefing_reference_str;
pub use commit_hash::persist_commit_hash_for_track;
pub use scope::{validate_review_group_name_str, validate_track_id_str};
pub use shared::{CodexReviewOutcome, build_review_v2_str};

// Crate-internal helpers used only by the CliApp impl methods in this file.
use approved::check_approved_str;
use briefing::get_briefing_for_scope_str;
use helpers::{
    build_base_prompt_from_input, is_safe_briefing_path, outcome_to_command_outcome,
    resolve_track_id_or_branch, resolve_track_id_or_branch_write, validate_all_paths,
};
use results::render_review_results_str;
use run::{run_claude_review_str, run_codex_review_str};
use scope::validate_scope_for_track_str;
use shared::{build_scope_query_interactor_no_diff_str, build_scope_query_interactor_str};

use std::path::PathBuf;
use std::time::{Duration, Instant};

use infrastructure::review_v2::{ClaudeReviewer, CodexReviewer};

use crate::{CommandOutcome, error::CompositionError};

pub use shim::ReviewCompositionRoot;

struct ReviewTelemetry<'a> {
    findings_count: u32,
    round_type: &'a str,
    verdict_parse_failed: bool,
    emit_subprocess: bool,
    subprocess_started_at: Option<Instant>,
}

fn review_telemetry_for_outcome<'a, E>(
    run_result: &'a Result<CodexReviewOutcome, E>,
    requested_round_type: &'a str,
) -> Option<ReviewTelemetry<'a>> {
    match run_result {
        Ok(CodexReviewOutcome::FinalCompleted {
            findings_count, subprocess_started_at, ..
        }) => Some(ReviewTelemetry {
            findings_count: *findings_count,
            round_type: "final",
            verdict_parse_failed: false,
            emit_subprocess: true,
            subprocess_started_at: Some(*subprocess_started_at),
        }),
        Ok(CodexReviewOutcome::FastCompleted { findings_count, subprocess_started_at, .. }) => {
            Some(ReviewTelemetry {
                findings_count: *findings_count,
                round_type: "fast",
                verdict_parse_failed: false,
                emit_subprocess: true,
                subprocess_started_at: Some(*subprocess_started_at),
            })
        }
        Ok(CodexReviewOutcome::Skipped { .. }) => Some(ReviewTelemetry {
            findings_count: 0,
            round_type: requested_round_type,
            verdict_parse_failed: false,
            emit_subprocess: false,
            subprocess_started_at: None,
        }),
        Ok(CodexReviewOutcome::SubprocessFailed {
            round_type,
            verdict_parse_failed,
            findings_count,
            subprocess_started_at,
            ..
        }) => Some(ReviewTelemetry {
            findings_count: *findings_count,
            round_type,
            verdict_parse_failed: *verdict_parse_failed,
            emit_subprocess: true,
            subprocess_started_at: Some(*subprocess_started_at),
        }),
        Err(_) => None,
    }
}

impl ReviewCompositionRoot {
    /// Run the local Codex-backed reviewer and auto-record verdict to review.json.
    ///
    /// Resolves `track_id` from the current git branch when `input.track_id` is
    /// `None`. Delegates to `run_codex_review_str` for all domain type handling
    /// (CN-02).
    ///
    /// # Errors
    /// Returns `Err` when arg validation, composition build, or the review cycle
    /// fails.
    pub fn review_run_codex(
        &self,
        input: ReviewRunCodexInput,
    ) -> Result<CommandOutcome, CompositionError> {
        let track_id = resolve_track_id_or_branch_write(input.track_id, &input.items_dir)?;

        validate_track_id_str(&track_id)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid --track-id: {e}")))?;
        validate_review_group_name_str(&input.group)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid --group: {e}")))?;

        let group = input.group.trim().to_owned();

        let maybe_briefing = get_briefing_for_scope_str(&group, &track_id, &input.items_dir)
            .map_err(CompositionError::Infrastructure)?;
        if let Some(path) = &maybe_briefing {
            if !is_safe_briefing_path(path) {
                eprintln!(
                    "[WARN] briefing_file for scope '{group}' contains unsafe characters — \
                     scope-specific severity policy injection skipped"
                );
            }
        }

        let mut base_prompt = build_base_prompt_from_input(input.briefing_file, input.prompt)?;
        append_scope_briefing_reference_str(
            &mut base_prompt,
            &group,
            &track_id,
            &input.items_dir,
            is_safe_briefing_path,
        )
        .map_err(CompositionError::Infrastructure)?;

        let timeout = Duration::from_secs(input.timeout_seconds);
        let reviewer =
            CodexReviewer::new(&input.model, timeout, base_prompt).with_scope_label(&group);

        let round_start = std::time::Instant::now();
        let run_result =
            run_codex_review_str(&track_id, &input.items_dir, &group, &input.round_type, reviewer);

        // Emit ReviewRound telemetry at the composition layer (T006 / AC-03 /
        // IN-03). Completed and SubprocessFailed outcomes also emit
        // ExternalSubprocess because the reviewer process was launched. Skipped
        // emits only ReviewRound with zero findings. Err remains a pre-subprocess
        // composition failure and does not emit.
        if let Some((ref w, ref tid)) =
            crate::telemetry_wiring::resolve_telemetry_writer_for_track(&input.items_dir, &track_id)
        {
            if let Some(telemetry) = review_telemetry_for_outcome(&run_result, &input.round_type) {
                crate::telemetry_wiring::emit_review_round(
                    w,
                    tid,
                    "codex",
                    &input.model,
                    telemetry.round_type,
                    telemetry.findings_count,
                    round_start,
                );
                if telemetry.emit_subprocess {
                    crate::telemetry_wiring::emit_external_subprocess(
                        w,
                        tid,
                        "codex",
                        0,
                        telemetry.verdict_parse_failed,
                        telemetry.subprocess_started_at.unwrap_or(round_start),
                    );
                }
            }
        }

        outcome_to_command_outcome(run_result.map_err(CompositionError::Usecase)?)
    }

    /// Run the local Claude-backed reviewer and auto-record verdict to review.json.
    ///
    /// Resolves `track_id` from the current git branch when `input.track_id` is
    /// `None`. Delegates to `run_claude_review_str` for all domain type handling
    /// (CN-02).
    ///
    /// # Errors
    /// Returns `Err` when arg validation, composition build, or the review cycle
    /// fails.
    pub fn review_run_claude(
        &self,
        input: ReviewRunClaudeInput,
    ) -> Result<CommandOutcome, CompositionError> {
        let track_id = resolve_track_id_or_branch_write(input.track_id, &input.items_dir)?;

        validate_track_id_str(&track_id)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid --track-id: {e}")))?;
        validate_review_group_name_str(&input.group)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid --group: {e}")))?;

        let group = input.group.trim().to_owned();

        let maybe_briefing = get_briefing_for_scope_str(&group, &track_id, &input.items_dir)
            .map_err(CompositionError::Infrastructure)?;
        if let Some(path) = &maybe_briefing {
            if !is_safe_briefing_path(path) {
                eprintln!(
                    "[WARN] briefing_file for scope '{group}' contains unsafe characters — \
                     scope-specific severity policy injection skipped"
                );
            }
        }

        let mut base_prompt = build_base_prompt_from_input(input.briefing_file, input.prompt)?;
        append_scope_briefing_reference_str(
            &mut base_prompt,
            &group,
            &track_id,
            &input.items_dir,
            is_safe_briefing_path,
        )
        .map_err(CompositionError::Infrastructure)?;

        let timeout = Duration::from_secs(input.timeout_seconds);
        let reviewer =
            ClaudeReviewer::new(&input.model, timeout, base_prompt).with_scope_label(&group);

        let round_start = std::time::Instant::now();
        let run_result =
            run_claude_review_str(&track_id, &input.items_dir, &group, &input.round_type, reviewer);

        // Emit review telemetry (T006 / AC-03 / IN-03).
        // See review_run_codex for the full rationale.
        if let Some((ref w, ref tid)) =
            crate::telemetry_wiring::resolve_telemetry_writer_for_track(&input.items_dir, &track_id)
        {
            if let Some(telemetry) = review_telemetry_for_outcome(&run_result, &input.round_type) {
                crate::telemetry_wiring::emit_review_round(
                    w,
                    tid,
                    "claude",
                    &input.model,
                    telemetry.round_type,
                    telemetry.findings_count,
                    round_start,
                );
                if telemetry.emit_subprocess {
                    crate::telemetry_wiring::emit_external_subprocess(
                        w,
                        tid,
                        "claude",
                        0,
                        telemetry.verdict_parse_failed,
                        telemetry.subprocess_started_at.unwrap_or(round_start),
                    );
                }
            }
        }

        outcome_to_command_outcome(run_result.map_err(CompositionError::Usecase)?)
    }

    /// Run the local reviewer with provider auto-resolved from agent-profiles.json.
    ///
    /// Resolves the `reviewer` capability from `agent-profiles.json` at the repo
    /// root, applies an optional model override, and dispatches to the appropriate
    /// reviewer implementation (codex or claude). Delegates all domain type
    /// handling to `run_codex_review_str` / `run_claude_review_str` (CN-02).
    ///
    /// # Errors
    /// Returns `Err` when profile loading, provider resolution, arg validation,
    /// or the review cycle fails.
    pub fn review_run_local(
        &self,
        input: ReviewRunLocalInput,
    ) -> Result<CommandOutcome, CompositionError> {
        let profiles = shared::load_agent_profiles_from_repo(Some(&input.items_dir))
            .map_err(|e| CompositionError::ConfigLoad(e.to_string()))?;
        let infra_round_type = shared::parse_round_type(&input.round_type)
            .map_err(|e| CompositionError::WiringFailed(e.to_string()))?;
        let mut resolved =
            profiles.resolve_execution("reviewer", infra_round_type).ok_or_else(|| {
                CompositionError::ConfigLoad(
                    "[ERROR] reviewer capability not defined in agent-profiles.json".to_owned(),
                )
            })?;

        if let Some(model_override) = input.model {
            resolved.model = Some(model_override);
        }

        eprintln!(
            "[sotp review local] provider={} model={}",
            resolved.provider,
            resolved.model.as_deref().unwrap_or("<none>")
        );

        let track_id = resolve_track_id_or_branch_write(input.track_id, &input.items_dir)?;
        let group = input.group.trim().to_owned();

        validate_track_id_str(&track_id)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid --track-id: {e}")))?;
        validate_review_group_name_str(&group)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid --group: {e}")))?;

        let maybe_briefing = get_briefing_for_scope_str(&group, &track_id, &input.items_dir)
            .map_err(CompositionError::Infrastructure)?;
        if let Some(path) = &maybe_briefing {
            if !is_safe_briefing_path(path) {
                eprintln!(
                    "[WARN] briefing_file for scope '{group}' contains unsafe characters — \
                     scope-specific severity policy injection skipped"
                );
            }
        }

        let mut base_prompt = build_base_prompt_from_input(input.briefing_file, input.prompt)?;
        append_scope_briefing_reference_str(
            &mut base_prompt,
            &group,
            &track_id,
            &input.items_dir,
            is_safe_briefing_path,
        )
        .map_err(CompositionError::Infrastructure)?;

        let timeout = Duration::from_secs(input.timeout_seconds);

        let round_start = std::time::Instant::now();
        let (run_result, provider_name, effective_model) = match resolved.provider.as_str() {
            "codex" => {
                let model = resolved.model.ok_or_else(|| {
                    CompositionError::ConfigLoad(
                        "[ERROR] codex reviewer requires a model (set model in agent-profiles.json)"
                            .to_owned(),
                    )
                })?;
                let reviewer =
                    CodexReviewer::new(&model, timeout, base_prompt).with_scope_label(&group);
                let result = run_codex_review_str(
                    &track_id,
                    &input.items_dir,
                    &group,
                    &input.round_type,
                    reviewer,
                );
                (result, "codex".to_owned(), model)
            }
            "claude" => {
                let model = resolved.model.ok_or_else(|| {
                    CompositionError::ConfigLoad(
                        "[ERROR] claude reviewer requires a model (set model in agent-profiles.json)"
                            .to_owned(),
                    )
                })?;
                let reviewer =
                    ClaudeReviewer::new(&model, timeout, base_prompt).with_scope_label(&group);
                let result = run_claude_review_str(
                    &track_id,
                    &input.items_dir,
                    &group,
                    &input.round_type,
                    reviewer,
                );
                (result, "claude".to_owned(), model)
            }
            other => {
                return Err(CompositionError::WiringFailed(format!(
                    "[ERROR] unsupported reviewer provider '{other}' \
                     (supported: 'codex', 'claude')"
                )));
            }
        };

        // Emit review telemetry (T006 / AC-03 / IN-03).
        // See review_run_codex for the full rationale.
        if let Some((ref w, ref tid)) =
            crate::telemetry_wiring::resolve_telemetry_writer_for_track(&input.items_dir, &track_id)
        {
            if let Some(telemetry) = review_telemetry_for_outcome(&run_result, &input.round_type) {
                crate::telemetry_wiring::emit_review_round(
                    w,
                    tid,
                    &provider_name,
                    &effective_model,
                    telemetry.round_type,
                    telemetry.findings_count,
                    round_start,
                );
                if telemetry.emit_subprocess {
                    crate::telemetry_wiring::emit_external_subprocess(
                        w,
                        tid,
                        &provider_name,
                        0,
                        telemetry.verdict_parse_failed,
                        telemetry.subprocess_started_at.unwrap_or(round_start),
                    );
                }
            }
        }

        outcome_to_command_outcome(run_result.map_err(CompositionError::Usecase)?)
    }

    /// Run the review-fix-lead fixer with provider auto-resolved from agent-profiles.json.
    ///
    /// Resolves the `review-fix-lead` capability from `agent-profiles.json` at the
    /// repo root. Supports only `"codex"` provider — constructs `CodexReviewFixRunner`
    /// and runs it through `RunReviewFixInteractor`. Unknown or unsupported providers
    /// return a clear error (mirrors `review_run_local` provider rejection).
    ///
    /// # Errors
    /// Returns `Err` when profile loading, provider resolution, arg validation,
    /// or the fix runner fails.
    pub fn review_run_fix_local(
        &self,
        input: RunReviewFixLocalInput,
    ) -> Result<CommandOutcome, CompositionError> {
        run_fix::run_fix_local(input).map_err(CompositionError::Infrastructure)
    }

    /// Run the review-fix-lead fixer, resolving `track_id` from the current
    /// git branch when omitted.
    ///
    /// Accepts an optional `track_id`. When `None`, performs branch-driven
    /// write-side resolution via `track_resolve_id_for_write` (fail-closed
    /// when not on a `track/<id>` branch). The caller (CLI handler) does not
    /// make the resolution / fail-closed decision — it is delegated here so
    /// the thin-bin layer stays free of orchestration logic.
    ///
    /// # Errors
    /// Returns `Err` when track ID resolution, profile loading, provider
    /// resolution, arg validation, or the fix runner fails.
    pub fn review_run_fix_local_resolve(
        &self,
        track_id_opt: Option<String>,
        scope: String,
        briefing_file: PathBuf,
        round_type: String,
        model: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        let track_id = resolve_track_id_or_branch_write(track_id_opt, &items_dir)?;
        let input = RunReviewFixLocalInput { scope, briefing_file, track_id, round_type, model };
        self.review_run_fix_local(input)
    }

    /// Check if the review state is approved and code hash is current.
    ///
    /// Resolves `track_id` from the current git branch when `None`. Delegates to
    /// `check_approved_str` for all domain type handling (CN-02).
    ///
    /// # Errors
    /// Returns `Err` when track ID resolution, store access, or approval
    /// evaluation fails.
    pub fn review_check_approved(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        use usecase::review_v2::ReviewApprovalDecision;

        let track_id = resolve_track_id_or_branch(track_id, &items_dir)?;
        let output = check_approved_str(&track_id, &items_dir)
            .map_err(|e| CompositionError::Usecase(format!("{e}")))?;

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
                    output.blocked_scopes.iter().map(|scope| format!("  {scope}")).collect();
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

        Ok(CommandOutcome { stdout: None, stderr: Some(msg), exit_code })
    }

    /// Show review results: per-scope state summary, optional round history.
    ///
    /// Resolves `track_id` from the current git branch when `None`. The
    /// `input.limit` field encodes: `0` = state summary only; `u32::MAX` = all
    /// rounds; any other value = up to that many rounds. Delegates to
    /// `render_review_results_str` for all domain type handling (CN-02).
    ///
    /// # Errors
    /// Returns `Err` when track ID resolution or review store access fails.
    pub fn review_results(
        &self,
        input: ReviewResultsInput,
    ) -> Result<CommandOutcome, CompositionError> {
        let track_id = resolve_track_id_or_branch(input.track_id, &input.items_dir)?;

        let limit = if input.limit == 0 { None } else { Some(input.limit) };

        let output = render_review_results_str(
            &track_id,
            &input.items_dir,
            input.scope.as_deref(),
            limit,
            &input.round_type,
            input.no_hint,
        )
        .map_err(CompositionError::Usecase)?;

        Ok(CommandOutcome::success(Some(output)))
    }

    /// Classify each given path into review scopes.
    ///
    /// Resolves `track_id` from the current git branch when `None`. Performs all
    /// path validation and scope classification without importing domain types in
    /// the method signature (CN-02).
    ///
    /// # Errors
    /// Returns `Err` when track ID resolution, path validation, scope config
    /// loading, or classification fails.
    pub fn review_classify(
        &self,
        paths: Vec<String>,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        use usecase::review_v2::ScopeQueryService as _;

        let track_id = resolve_track_id_or_branch(track_id, &items_dir)?;

        validate_all_paths(&paths)?;

        let interactor = build_scope_query_interactor_no_diff_str(&track_id, &items_dir)
            .map_err(|e| CompositionError::WiringFailed(e.to_string()))?;

        let classifications = interactor
            .classify_by_strings(paths)
            .map_err(|e| CompositionError::Usecase(format!("classify failed: {e}")))?;

        let mut out = String::new();
        for entry in &classifications {
            use std::fmt::Write as _;
            let scope = entry.scopes.join(",");
            let _ = writeln!(out, "{path}\t{scope}", path = entry.path);
        }

        Ok(CommandOutcome::success(Some(out)))
    }

    /// List the diff files belonging to the given scope.
    ///
    /// Validates the scope name before any diff I/O (AC-08). Resolves `track_id`
    /// from the current git branch when `None`. Delegates to
    /// `build_scope_query_interactor_str` for diff resolution (CN-02).
    ///
    /// # Errors
    /// Returns `Err` when track ID resolution, scope validation, diff resolution,
    /// or file listing fails.
    pub fn review_files(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        use usecase::review_v2::{ScopeQueryError, ScopeQueryService as _};

        let track_id = resolve_track_id_or_branch(track_id, &items_dir)?;

        validate_scope_for_track_str(&track_id, &items_dir, &scope)
            .map_err(CompositionError::WiringFailed)?;

        let interactor = build_scope_query_interactor_str(&track_id, &items_dir)
            .map_err(|e| CompositionError::WiringFailed(e.to_string()))?;
        let files = interactor.files_by_string(scope).map_err(|err| match err {
            ScopeQueryError::DiffGet(inner) => {
                CompositionError::Usecase(format!("diff getter failed: {inner}"))
            }
            ScopeQueryError::UnknownScope(s) => {
                CompositionError::Usecase(format!("Unknown scope: {s}"))
            }
            ScopeQueryError::InvalidPath { path, reason } => {
                CompositionError::Usecase(format!("invalid path '{path}': {reason}"))
            }
            ScopeQueryError::InvalidScopeName { name, reason } => {
                CompositionError::Usecase(format!("invalid scope name '{name}': {reason}"))
            }
        })?;

        let mut out = String::new();
        for file in &files {
            use std::fmt::Write as _;
            let _ = writeln!(out, "{file}");
        }

        Ok(CommandOutcome::success(Some(out)))
    }

    /// Validate a scope name for the given track.
    ///
    /// Resolves `track_id` from the current git branch when `None`. Returns a
    /// success `CommandOutcome` if the scope is valid, `Err` otherwise (CN-02).
    ///
    /// # Errors
    /// Returns `Err` when track ID resolution or scope validation fails.
    pub fn review_validate_scope(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        let track_id = resolve_track_id_or_branch(track_id, &items_dir)?;
        validate_scope_for_track_str(&track_id, &items_dir, &scope)
            .map_err(CompositionError::WiringFailed)?;
        Ok(CommandOutcome::success(None))
    }

    /// Get the briefing for a review scope.
    ///
    /// Resolves `track_id` from the current git branch when `None`. Returns the
    /// configured briefing file path as stdout, or an empty success when no
    /// briefing is configured for the scope (CN-02).
    ///
    /// # Errors
    /// Returns `Err` when track ID resolution or scope config loading fails.
    pub fn review_get_briefing(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        let track_id = resolve_track_id_or_branch(track_id, &items_dir)?;
        let maybe_path = get_briefing_for_scope_str(&scope, &track_id, &items_dir)
            .map_err(CompositionError::Infrastructure)?;
        Ok(CommandOutcome::success(maybe_path))
    }

    /// Persist a commit hash for the review cycle.
    ///
    /// Resolves `track_id` from the current git branch when `None`. Delegates to
    /// `infrastructure::review_v2::persist_commit_hash_for_track` for all domain
    /// and I/O operations (CN-02). The `items_dir` parameter is accepted for API
    /// consistency but the infrastructure function always uses the canonical
    /// `track/items` path under the repo root.
    ///
    /// # Errors
    /// Returns `Err` when track ID resolution, branch guard, git, or I/O fails.
    pub fn review_persist_commit_hash(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        let track_id = resolve_track_id_or_branch(track_id, &items_dir)?;
        let head_sha =
            persist_commit_hash_for_track(&track_id).map_err(CompositionError::Infrastructure)?;
        eprintln!("[review] Recorded .commit_hash: {head_sha}");
        Ok(CommandOutcome::success(None))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    use crate::review_v2::process_guards::{CwdGuard, EnvGuard, GitRunner};
    #[cfg(unix)]
    use crate::test_support::make_executable;

    /// Serializes tests in this module that mutate the process CWD.
    /// Note: nextest runs each test in its own process, so this lock guards
    /// against races only when tests run in a shared process (e.g., `cargo test`).
    fn cwd_lock() -> &'static std::sync::Mutex<()> {
        crate::test_support::process_env_lock()
    }

    fn git_stdout(root: &std::path::Path, args: &[&str]) -> String {
        let output = Command::new("git").args(args).current_dir(root).output().unwrap();
        assert!(
            output.status.success(),
            "git command failed: git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    struct ReviewEntrypointRepo {
        _dir: tempfile::TempDir,
        items_dir: PathBuf,
        track_dir: PathBuf,
        track_id: String,
    }

    fn setup_review_entrypoint_repo(track_id: &str) -> ReviewEntrypointRepo {
        let dir = tempfile::tempdir().unwrap();
        GitRunner::at(dir.path()).assert_success(&["init", "-b", "main"]);
        GitRunner::at(dir.path()).assert_success(&["config", "user.email", "test@example.com"]);
        GitRunner::at(dir.path()).assert_success(&["config", "user.name", "Test"]);

        let track_root = dir.path().join("track");
        fs::create_dir_all(track_root.join("items")).unwrap();
        let config_dir = dir.path().join(".harness/config");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("review-scope.json"),
            r#"{"version":2,"groups":{"cli_composition":{"patterns":["src/**"]}}}"#,
        )
        .unwrap();
        fs::write(
            config_dir.join("branch-strategy.json"),
            r#"{"base_branch":"main","merge_target":"main","merge_method":"squash"}"#,
        )
        .unwrap();
        fs::write(dir.path().join("README.md"), "init\n").unwrap();
        GitRunner::at(dir.path()).assert_success(&["add", "."]);
        GitRunner::at(dir.path()).assert_success(&["commit", "-m", "base"]);
        let base_sha = git_stdout(dir.path(), &["rev-parse", "HEAD"]);

        GitRunner::at(dir.path()).assert_success(&["checkout", "-b", &format!("track/{track_id}")]);
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "pub fn changed() {}\n").unwrap();
        GitRunner::at(dir.path()).assert_success(&["add", "src/lib.rs"]);
        GitRunner::at(dir.path()).assert_success(&["commit", "-m", "change src"]);

        let items_dir = track_root.join("items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join(".commit_hash"), base_sha).unwrap();

        ReviewEntrypointRepo { _dir: dir, items_dir, track_dir, track_id: track_id.to_owned() }
    }

    struct TrackBranchRepo {
        _dir: tempfile::TempDir,
        items_dir: PathBuf,
    }

    fn setup_track_branch_repo(track_id: &str) -> TrackBranchRepo {
        let dir = tempfile::tempdir().unwrap();
        GitRunner::at(dir.path()).assert_success(&["init", "-b", "main"]);
        GitRunner::at(dir.path()).assert_success(&["config", "user.email", "test@example.com"]);
        GitRunner::at(dir.path()).assert_success(&["config", "user.name", "Test"]);
        fs::write(dir.path().join("README.md"), "init\n").unwrap();
        GitRunner::at(dir.path()).assert_success(&["add", "README.md"]);
        GitRunner::at(dir.path()).assert_success(&["commit", "-m", "init"]);
        GitRunner::at(dir.path()).assert_success(&["checkout", "-b", &format!("track/{track_id}")]);

        let items_dir = dir.path().join("track/items");
        fs::create_dir_all(&items_dir).unwrap();

        TrackBranchRepo { _dir: dir, items_dir }
    }

    #[test]
    fn test_review_telemetry_for_outcome_skipped_returns_zero_findings_without_subprocess() {
        let run_result: Result<super::CodexReviewOutcome, super::shared::ReviewSharedError> =
            Ok(super::CodexReviewOutcome::Skipped { scope_label: "cli_composition".to_owned() });

        let telemetry = super::review_telemetry_for_outcome(&run_result, "fast").unwrap();

        assert_eq!(telemetry.findings_count, 0);
        assert_eq!(telemetry.round_type, "fast");
        assert!(!telemetry.verdict_parse_failed);
        assert!(!telemetry.emit_subprocess);
    }

    fn write_agent_profiles(root: &std::path::Path, provider: &str) {
        let config_dir = root.join(".harness/config");
        fs::create_dir_all(&config_dir).unwrap();
        let content = format!(
            r#"{{
  "schema_version": 1,
  "providers": {{
    "codex": {{ "label": "Codex" }},
    "{provider}": {{ "label": "Test Provider" }}
  }},
  "capabilities": {{
    "reviewer": {{
      "provider": "{provider}",
      "model": "review-final",
      "fast_provider": "{provider}",
      "fast_model": "review-fast"
    }},
    "review-fix-lead": {{
      "provider": "{provider}",
      "model": "gpt-final",
      "fast_provider": "{provider}",
      "fast_model": "gpt-fast"
    }}
  }}
}}
"#
        );
        fs::write(config_dir.join("agent-profiles.json"), content).unwrap();
    }

    #[cfg(unix)]
    fn write_fake_codex_bin_with_body(bin_dir: &std::path::Path, body: &str) {
        fs::create_dir_all(bin_dir).unwrap();
        let codex = bin_dir.join("codex");
        let script = format!(
            r#"#!/bin/sh
case "$1" in
  --version)
    echo "codex 0.125.0"
    exit 0
    ;;
esac
out="${{11}}"
if [ -z "$out" ]; then
  echo "missing output-last-message" >&2
  exit 9
fi

{body}
"#
        );
        fs::write(&codex, script).unwrap();
        make_executable(&codex);
    }

    #[cfg(unix)]
    fn write_fake_codex_bin(bin_dir: &std::path::Path) {
        write_fake_codex_bin_with_body(
            bin_dir,
            r#"cat >/dev/null
printf 'REVIEW_FIX_STATUS: completed\n' > "$out"
printf 'fake stdout\n'
exit 0
"#,
        );
    }

    #[cfg(unix)]
    fn write_fake_codex_reviewer_bin(bin_dir: &std::path::Path) {
        write_fake_codex_bin_with_body(
            bin_dir,
            r#"
printf '{"verdict":"zero_findings","findings":[]}\n' > "$out"
exit 0
"#,
        );
    }

    #[cfg(unix)]
    fn write_fake_claude_reviewer_bin(bin_dir: &std::path::Path) {
        fs::create_dir_all(bin_dir).unwrap();
        let claude = bin_dir.join("claude");
        let script = r#"#!/bin/sh
printf '%s\n' '{"type":"result","structured_output":{"verdict":"zero_findings","findings":[]}}'
exit 0
"#;
        fs::write(&claude, script).unwrap();
        make_executable(&claude);
    }

    fn prepend_path(bin_dir: &std::path::Path) -> EnvGuard {
        let previous_path = std::env::var_os("PATH").unwrap_or_default();
        let mut test_path = bin_dir.as_os_str().to_os_string();
        test_path.push(":");
        test_path.push(previous_path);
        EnvGuard::set("PATH", test_path)
    }

    fn assert_review_telemetry(
        track_dir: &std::path::Path,
        provider: &str,
        model: &str,
        command: &str,
        round_type: &str,
    ) {
        let telemetry_path = track_dir.join("logs/telemetry.jsonl");
        let content = fs::read_to_string(&telemetry_path).unwrap();
        let events: Vec<serde_json::Value> =
            content.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

        assert!(
            events.iter().any(|event| {
                event.get("event_type").and_then(serde_json::Value::as_str) == Some("ReviewRound")
                    && event.get("provider").and_then(serde_json::Value::as_str) == Some(provider)
                    && event.get("model").and_then(serde_json::Value::as_str) == Some(model)
                    && event.get("round_type").and_then(serde_json::Value::as_str)
                        == Some(round_type)
                    && event.get("findings_count").and_then(serde_json::Value::as_u64) == Some(0)
            }),
            "ReviewRound telemetry missing from {content}"
        );
        assert!(
            events.iter().any(|event| {
                event.get("event_type").and_then(serde_json::Value::as_str)
                    == Some("ExternalSubprocess")
                    && event.get("command").and_then(serde_json::Value::as_str) == Some(command)
                    && event.get("retry_count").and_then(serde_json::Value::as_u64) == Some(0)
                    && event.get("verdict_parse_failed").and_then(serde_json::Value::as_bool)
                        == Some(false)
            }),
            "ExternalSubprocess telemetry missing from {content}"
        );
    }

    fn run_review_fix_input(briefing_file: PathBuf) -> crate::review_v2::RunReviewFixLocalInput {
        crate::review_v2::RunReviewFixLocalInput {
            scope: "cli_composition".to_owned(),
            briefing_file,
            track_id: "review-fix-codex-rustify-2026-05-31".to_owned(),
            round_type: "fast".to_owned(),
            model: Some("gpt-5.5".to_owned()),
        }
    }

    /// Pin the regression: `resolve_track_id_from_branch` must anchor git discovery
    /// to the project root (derived by stripping `track/items`), NOT to `items_dir`
    /// directly.  This test invokes the function with the **relative** path
    /// `"track/items"` from a **subdirectory** of the repo root to reproduce the
    /// actual failure mode.
    ///
    /// Before the fix, `discover_from("track/items")` ran `git -C track/items …`
    /// from the subdirectory CWD where `track/items` does not exist as a path,
    /// causing git to fail.  After the fix, `resolve_project_root("track/items")`
    /// returns `"."` and `discover_from(".")` succeeds from any directory inside
    /// the repo.
    #[test]
    fn resolve_track_id_from_branch_relative_items_dir_works_from_subdirectory() {
        let _lock = cwd_lock().lock().unwrap();

        let repo = setup_track_branch_repo("test-track");

        // Create a subdirectory inside the repo.  From this path, the relative string
        // "track/items" does NOT point to an existing directory, so the pre-fix code
        // (`discover_from("track/items")`) would run `git -C track/items …` and fail.
        let subdir = repo._dir.path().join("src");
        fs::create_dir_all(&subdir).unwrap();

        // Restore CWD on drop, even if an assertion panics.
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(&subdir).unwrap();

        // Pass the relative path — the function must succeed by anchoring to CWD (".").
        let result =
            super::helpers::resolve_track_id_from_branch(std::path::Path::new("track/items"));

        assert_eq!(result.unwrap(), "test-track");
    }

    #[cfg(unix)]
    #[test]
    fn review_run_codex_happy_path_writes_verdict_and_telemetry() {
        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-run-codex-2026");
        let bin_dir = repo.track_dir.join("fake-bin-codex");
        write_fake_codex_reviewer_bin(&bin_dir);
        let _path_guard = prepend_path(&bin_dir);
        let _telemetry_guard = EnvGuard::set("SOTP_TELEMETRY", OsString::from("1"));
        let _telemetry_dir_guard = EnvGuard::remove("SOTP_TELEMETRY_DIR");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        let outcome = crate::review_v2::ReviewCompositionRoot::new()
            .review_run_codex(crate::review_v2::ReviewRunCodexInput {
                model: "codex-review-model".to_owned(),
                timeout_seconds: 10,
                briefing_file: None,
                prompt: Some("Review.".to_owned()),
                track_id: Some(repo.track_id.clone()),
                round_type: "fast".to_owned(),
                group: "cli_composition".to_owned(),
                items_dir: repo.items_dir.clone(),
            })
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.as_deref().unwrap_or("").contains("zero_findings"));
        assert!(repo.track_dir.join("review.json").exists());
        assert_review_telemetry(&repo.track_dir, "codex", "codex-review-model", "codex", "fast");
    }

    #[cfg(unix)]
    #[test]
    fn review_run_claude_happy_path_writes_verdict_and_telemetry() {
        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-run-claude-2026");
        let bin_dir = repo.track_dir.join("fake-bin-claude");
        write_fake_claude_reviewer_bin(&bin_dir);
        let _path_guard = prepend_path(&bin_dir);
        let _telemetry_guard = EnvGuard::set("SOTP_TELEMETRY", OsString::from("1"));
        let _telemetry_dir_guard = EnvGuard::remove("SOTP_TELEMETRY_DIR");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        let outcome = crate::review_v2::ReviewCompositionRoot::new()
            .review_run_claude(crate::review_v2::ReviewRunClaudeInput {
                model: "claude-review-model".to_owned(),
                timeout_seconds: 10,
                briefing_file: None,
                prompt: Some("Review.".to_owned()),
                track_id: Some(repo.track_id.clone()),
                round_type: "fast".to_owned(),
                group: "cli_composition".to_owned(),
                items_dir: repo.items_dir.clone(),
            })
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.as_deref().unwrap_or("").contains("zero_findings"));
        assert!(repo.track_dir.join("review.json").exists());
        assert_review_telemetry(&repo.track_dir, "claude", "claude-review-model", "claude", "fast");
    }

    #[cfg(unix)]
    #[test]
    fn review_run_local_resolves_profile_happy_path_writes_verdict_and_telemetry() {
        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-run-local-2026");
        write_agent_profiles(repo._dir.path(), "claude");
        let bin_dir = repo.track_dir.join("fake-bin-local");
        write_fake_claude_reviewer_bin(&bin_dir);
        let _path_guard = prepend_path(&bin_dir);
        let _telemetry_guard = EnvGuard::set("SOTP_TELEMETRY", OsString::from("1"));
        let _telemetry_dir_guard = EnvGuard::remove("SOTP_TELEMETRY_DIR");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        let outcome = crate::review_v2::ReviewCompositionRoot::new()
            .review_run_local(crate::review_v2::ReviewRunLocalInput {
                model: None,
                timeout_seconds: 10,
                briefing_file: None,
                prompt: Some("Review.".to_owned()),
                track_id: Some(repo.track_id.clone()),
                round_type: "fast".to_owned(),
                group: "cli_composition".to_owned(),
                items_dir: repo.items_dir.clone(),
            })
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.as_deref().unwrap_or("").contains("zero_findings"));
        assert!(repo.track_dir.join("review.json").exists());
        assert_review_telemetry(&repo.track_dir, "claude", "review-fast", "claude", "fast");
    }

    #[cfg(unix)]
    #[test]
    fn review_run_fix_local_codex_completed_status_returns_command_outcome() {
        let _lock = cwd_lock().lock().unwrap();

        let dir = tempfile::tempdir().unwrap();
        GitRunner::at(dir.path()).assert_success(&["init", "-b", "main"]);
        write_agent_profiles(dir.path(), "codex");
        let briefing = dir.path().join("briefing.md");
        fs::write(&briefing, "# Briefing\n").unwrap();

        let bin_dir = dir.path().join("bin-test");
        write_fake_codex_bin(&bin_dir);
        let previous_path = std::env::var_os("PATH").unwrap_or_default();
        let mut test_path = bin_dir.as_os_str().to_os_string();
        test_path.push(":");
        test_path.push(previous_path);
        let _path_guard = EnvGuard::set("PATH", test_path);
        let _sandbox_guard = EnvGuard::remove("CODEX_SANDBOX");

        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(dir.path()).unwrap();

        let outcome = crate::review_v2::ReviewCompositionRoot::new()
            .review_run_fix_local(run_review_fix_input(briefing))
            .unwrap();

        assert_eq!(outcome.stdout.as_deref(), Some("REVIEW_FIX_STATUS: completed"));
        assert_eq!(outcome.stderr, None);
        assert_eq!(outcome.exit_code, 0);
    }

    #[test]
    fn review_run_fix_local_claude_provider_returns_subagent_dispatch_instruction() {
        // PR #175 follow-up: review-fix-lead.provider = "claude" must return a
        // structured dispatch instruction (stdout sentinel + JSON, exit code
        // SUBAGENT_DISPATCH_EXIT_CODE), not an error, so the orchestrator can
        // route to the Claude Code subagent without provider conditionals.
        let _lock = cwd_lock().lock().unwrap();

        let dir = tempfile::tempdir().unwrap();
        GitRunner::at(dir.path()).assert_success(&["init", "-b", "main"]);
        write_agent_profiles(dir.path(), "claude");
        let briefing = dir.path().join("briefing.md");
        fs::write(&briefing, "# Briefing\n").unwrap();

        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(dir.path()).unwrap();

        let outcome = crate::review_v2::ReviewCompositionRoot::new()
            .review_run_fix_local(run_review_fix_input(briefing.clone()))
            .expect("claude provider must succeed with a dispatch instruction");

        assert_eq!(
            outcome.exit_code,
            crate::review_v2::run_fix::SUBAGENT_DISPATCH_EXIT_CODE,
            "claude provider must exit with SUBAGENT_DISPATCH_EXIT_CODE"
        );
        let stdout = outcome.stdout.expect("dispatch instruction must be on stdout");
        let mut lines = stdout.lines();
        let sentinel = lines.next().expect("first stdout line must be the dispatch sentinel");
        assert_eq!(sentinel, crate::review_v2::run_fix::SUBAGENT_DISPATCH_SENTINEL);
        let json = lines.next().expect("second stdout line must be the dispatch JSON payload");
        assert!(json.contains("\"agent\":\"review-fix-lead\""), "JSON must name the agent: {json}");
        assert!(json.contains("\"scope\":\"cli_composition\""), "JSON must carry scope: {json}");
        assert!(
            json.contains(&format!("\"briefing_file\":\"{}\"", briefing.display())),
            "JSON must carry briefing_file: {json}"
        );
        assert!(
            json.contains("\"track_id\":\"review-fix-codex-rustify-2026-05-31\""),
            "JSON must carry track_id: {json}"
        );
        assert!(json.contains("\"round_type\":\"fast\""), "JSON must carry round_type: {json}");
    }

    /// Regression: exit 64 + `SUBAGENT_DISPATCH_REQUIRED` sentinel must pass through
    /// the full `ReviewDriver` → `ReviewServiceImpl` chain unchanged when
    /// `review-fix-lead.provider` is `"claude"`.
    ///
    /// Before the fix, `ReviewServiceImpl::run_fix_local` mapped exit 64 to
    /// `status: "failed"` and the driver then rewrote stdout to
    /// `"REVIEW_FIX_STATUS: failed"` with exit code 1, so the orchestrator never
    /// saw the dispatch sentinel and could not launch the Claude subagent.
    #[test]
    fn review_driver_handle_claude_provider_passes_through_subagent_dispatch_sentinel() {
        let _lock = cwd_lock().lock().unwrap();

        let dir = tempfile::tempdir().unwrap();
        GitRunner::at(dir.path()).assert_success(&["init", "-b", "main"]);
        write_agent_profiles(dir.path(), "claude");
        let briefing = dir.path().join("briefing.md");
        fs::write(&briefing, "# Briefing\n").unwrap();

        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(dir.path()).unwrap();

        let input = cli_driver::review::ReviewInput::RunFixLocal {
            scope: "cli_composition".to_owned(),
            briefing_file: briefing,
            track_id: "review-fix-codex-rustify-2026-05-31".to_owned(),
            round_type: "fast".to_owned(),
            model: Some("gpt-5.5".to_owned()),
        };
        let outcome = crate::review_v2::ReviewCompositionRoot::new().review_driver().handle(input);

        assert_eq!(
            outcome.exit_code,
            crate::review_v2::run_fix::SUBAGENT_DISPATCH_EXIT_CODE,
            "driver must pass through SUBAGENT_DISPATCH_EXIT_CODE (64) for claude provider; \
             got {} — was it remapped to 1 by the failed-status path?",
            outcome.exit_code
        );
        let stdout = outcome.stdout.expect("dispatch sentinel must appear on stdout");
        assert!(
            stdout.starts_with(crate::review_v2::run_fix::SUBAGENT_DISPATCH_SENTINEL),
            "stdout first line must be SUBAGENT_DISPATCH_SENTINEL, got: {stdout:?}"
        );
        assert!(
            !stdout.contains("REVIEW_FIX_STATUS:"),
            "driver must NOT rewrite sentinel to REVIEW_FIX_STATUS line, got: {stdout:?}"
        );
    }

    #[test]
    fn review_run_claude_returns_branch_error_not_discovery_error_for_non_track_branch() {
        let dir = tempfile::tempdir().unwrap();
        GitRunner::at(dir.path()).assert_success(&["init", "-b", "main"]);
        GitRunner::at(dir.path()).assert_success(&["config", "user.email", "test@example.com"]);
        GitRunner::at(dir.path()).assert_success(&["config", "user.name", "Test"]);
        fs::write(dir.path().join("README.md"), "init\n").unwrap();
        GitRunner::at(dir.path()).assert_success(&["add", "README.md"]);
        GitRunner::at(dir.path()).assert_success(&["commit", "-m", "init"]);
        let items_dir = dir.path().join("track/items");
        fs::create_dir_all(&items_dir).unwrap();

        let result = crate::review_v2::ReviewCompositionRoot::new().review_run_claude(
            crate::review_v2::ReviewRunClaudeInput {
                model: "test-model".to_owned(),
                timeout_seconds: 10,
                briefing_file: None,
                prompt: Some("Review.".to_owned()),
                track_id: None,
                round_type: "fast".to_owned(),
                group: "cli_composition".to_owned(),
                items_dir,
            },
        );

        assert!(result.is_err(), "expected Err on non-track branch, got Ok");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not a track branch") || msg.contains("main"),
            "expected branch error, got: {msg}"
        );
    }

    #[test]
    fn review_run_local_unsupported_provider_returns_error() {
        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-run-local-unsupported-2026");
        write_agent_profiles(repo._dir.path(), "gemini");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        let result = crate::review_v2::ReviewCompositionRoot::new().review_run_local(
            crate::review_v2::ReviewRunLocalInput {
                model: None,
                timeout_seconds: 10,
                briefing_file: None,
                prompt: Some("Review.".to_owned()),
                track_id: Some(repo.track_id.clone()),
                round_type: "fast".to_owned(),
                group: "cli_composition".to_owned(),
                items_dir: repo.items_dir.clone(),
            },
        );

        assert!(result.is_err(), "expected unsupported provider error, got: {result:?}");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("unsupported reviewer provider 'gemini'"),
            "expected unsupported provider error, got: {msg}"
        );
    }

    /// Pin that `resolve_track_id_from_branch` returns an error for a relative
    /// `items_dir` that does not follow the `*/track/items` structure.
    #[test]
    fn resolve_track_id_from_branch_rejects_non_canonical_items_dir() {
        // A path like "wrong/path" does not end in "track/items", so
        // resolve_project_root should return an error before any git I/O.
        let result =
            super::helpers::resolve_track_id_from_branch(std::path::Path::new("wrong/path"));
        assert!(result.is_err(), "expected error for non-canonical items_dir, got: {result:?}");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("track/items"), "error should mention 'track/items', got: {msg}");
    }

    /// Pin the absolute path case: an absolute `items_dir` must also anchor
    /// git discovery to the derived project root, not directly to `items_dir`.
    #[test]
    fn resolve_track_id_from_branch_works_with_absolute_items_dir() {
        let repo = setup_track_branch_repo("abs-track");

        // Pass the absolute path directly — no CWD dependency.
        let result = super::helpers::resolve_track_id_from_branch(&repo.items_dir);

        assert_eq!(result.unwrap(), "abs-track");
    }

    /// Pin that the path passed to `resolve_track_id_from_branch` is used as
    /// `items_dir`.  When the canonical path exists as an absolute dir but no
    /// track branch is active, the function must fail with a branch error (not a
    /// git-discovery error), confirming that git is discovered successfully.
    ///
    /// This test passes an absolute `items_dir` and never changes the process CWD,
    /// so it does not hold `cwd_lock`.
    #[test]
    fn resolve_track_id_from_branch_returns_branch_error_on_non_track_branch() {
        let dir = tempfile::tempdir().unwrap();
        GitRunner::at(dir.path()).assert_success(&["init", "-b", "main"]);
        GitRunner::at(dir.path()).assert_success(&["config", "user.email", "test@example.com"]);
        GitRunner::at(dir.path()).assert_success(&["config", "user.name", "Test"]);
        fs::write(dir.path().join("README.md"), "init\n").unwrap();
        GitRunner::at(dir.path()).assert_success(&["add", "README.md"]);
        GitRunner::at(dir.path()).assert_success(&["commit", "-m", "init"]);
        // Stay on `main` (not a track branch).

        let items_dir = dir.path().join("track/items");
        fs::create_dir_all(&items_dir).unwrap();

        let result = super::helpers::resolve_track_id_from_branch(&items_dir);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        // The error must mention the branch name, not a git-discovery failure.
        assert!(
            msg.contains("not a track branch") || msg.contains("main"),
            "expected branch error, got: {msg}"
        );
    }

    #[test]
    fn validate_all_paths_accepts_clean_relative_paths() {
        let result = super::helpers::validate_all_paths(&[
            "src/lib.rs".to_owned(),
            "apps/cli/mod.rs".to_owned(),
        ]);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_all_paths_rejects_absolute_paths() {
        let result = super::helpers::validate_all_paths(&["/etc/passwd".to_owned()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("absolute paths"));
    }

    #[test]
    fn validate_all_paths_rejects_windows_drive_prefixed_paths() {
        for raw in ["C:", "C:foo", "C:/foo", "C:\\foo", "z:relative"] {
            let result = super::helpers::validate_all_paths(&[raw.to_owned()]);
            assert!(result.is_err(), "expected drive-prefixed path to be rejected: {raw}");
            assert!(result.unwrap_err().to_string().contains("absolute paths"));
        }
    }

    #[test]
    fn validate_all_paths_rejects_traversal_components() {
        let result = super::helpers::validate_all_paths(&["../../etc/passwd".to_owned()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("traversal"));
    }

    #[test]
    fn is_safe_briefing_path_rejects_empty() {
        assert!(!super::helpers::is_safe_briefing_path(""));
    }

    #[test]
    fn is_safe_briefing_path_rejects_absolute_unix() {
        assert!(!super::helpers::is_safe_briefing_path("/tmp/brief.md"));
    }

    #[test]
    fn is_safe_briefing_path_rejects_traversal() {
        assert!(!super::helpers::is_safe_briefing_path("../some/brief.md"));
    }

    #[test]
    fn is_safe_briefing_path_accepts_relative_clean_path() {
        assert!(super::helpers::is_safe_briefing_path("track/items/my-track/briefing.md"));
    }

    /// Confirm that `PathBuf` passed as `items_dir` is handled correctly for
    /// both read-path (explicit short-circuit) and non-canonical (error) cases,
    /// without requiring a live git repo.
    #[test]
    fn resolve_track_id_or_branch_explicit_id_bypasses_items_dir_validation() {
        // Even a clearly non-canonical items_dir is ignored when track_id is explicit.
        let result = super::helpers::resolve_track_id_or_branch(
            Some("my-track".to_owned()),
            std::path::Path::new("not/track/items"),
        );
        assert_eq!(result.unwrap(), "my-track");
    }

    #[test]
    fn resolve_track_id_or_branch_none_id_validates_items_dir_structure() {
        // When track_id is None, items_dir must follow the canonical `*/track/items` structure.
        // Use a path that genuinely does NOT end in `track/items`.
        let result = super::helpers::resolve_track_id_or_branch(
            None,
            std::path::Path::new("wrong/path/here"),
        );
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("track/items"), "expected items-dir error, got: {msg}");
    }

    /// Pin the regression on the public API path (`review_run_codex`).
    ///
    /// `resolve_track_id_or_branch_write` is called as the first step of
    /// `review_run_codex`.  It must anchor git discovery to the project root
    /// (via `resolve_project_root`), not to `items_dir` directly.  When the
    /// function succeeds in discovering the repo but finds a non-track branch,
    /// it returns a **branch** error — not a filesystem error about `items_dir`.
    ///
    /// This test uses an absolute `items_dir` (no CWD mutation) to verify that
    /// the public entrypoint path correctly reaches the branch-guard logic.
    #[test]
    fn review_run_codex_returns_branch_error_not_discovery_error_for_non_track_branch() {
        let dir = tempfile::tempdir().unwrap();
        GitRunner::at(dir.path()).assert_success(&["init", "-b", "main"]);
        GitRunner::at(dir.path()).assert_success(&["config", "user.email", "test@example.com"]);
        GitRunner::at(dir.path()).assert_success(&["config", "user.name", "Test"]);
        fs::write(dir.path().join("README.md"), "init\n").unwrap();
        GitRunner::at(dir.path()).assert_success(&["add", "README.md"]);
        GitRunner::at(dir.path()).assert_success(&["commit", "-m", "init"]);
        // Stay on `main` (not a track branch).

        let items_dir = dir.path().join("track/items");
        fs::create_dir_all(&items_dir).unwrap();

        let app = crate::review_v2::ReviewCompositionRoot::new();
        let input = crate::review_v2::ReviewRunCodexInput {
            model: "test-model".to_owned(),
            timeout_seconds: 10,
            briefing_file: None,
            prompt: Some("Review.".to_owned()),
            track_id: None,
            round_type: "fast".to_owned(),
            group: "cli_composition".to_owned(),
            items_dir,
        };

        let result = app.review_run_codex(input);

        assert!(result.is_err(), "expected Err on non-track branch, got Ok");
        let msg = result.unwrap_err().to_string();
        // The error must be a branch error ("not a track branch", "main", or similar)
        // rather than a git-discovery error ("failed to run git", "No such file", etc.).
        assert!(
            msg.contains("not a track branch") || msg.contains("main"),
            "expected branch error, got: {msg}"
        );
    }
}
