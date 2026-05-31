//! `review_v2` command family — composition logic and CliApp impl methods.

pub(crate) mod approved;
pub(crate) mod briefing;
pub(crate) mod commit_hash;
mod inputs;
pub(crate) mod null_reviewer;
pub(crate) mod results;
pub(crate) mod run;
pub(crate) mod scope;
pub(crate) mod shared;

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
use results::render_review_results_str;
use run::{run_claude_review_str, run_codex_review_str};
use scope::validate_scope_for_track_str;
use shared::{build_scope_query_interactor_no_diff_str, build_scope_query_interactor_str};

use std::path::PathBuf;
use std::time::Duration;

use infrastructure::review_v2::{ClaudeReviewer, CodexReviewFixRunner, CodexReviewer};

use crate::{CliApp, CommandOutcome};

impl CliApp {
    /// Run the local Codex-backed reviewer and auto-record verdict to review.json.
    ///
    /// Resolves `track_id` from the current git branch when `input.track_id` is
    /// `None`. Delegates to `run_codex_review_str` for all domain type handling
    /// (CN-02).
    ///
    /// # Errors
    /// Returns `Err` when arg validation, composition build, or the review cycle
    /// fails.
    pub fn review_run_codex(&self, input: ReviewRunCodexInput) -> Result<CommandOutcome, String> {
        let track_id = resolve_track_id_or_branch_write(input.track_id, &input.items_dir)?;

        validate_track_id_str(&track_id).map_err(|e| format!("invalid --track-id: {e}"))?;
        validate_review_group_name_str(&input.group)
            .map_err(|e| format!("invalid --group: {e}"))?;

        let group = input.group.trim().to_owned();

        let maybe_briefing = get_briefing_for_scope_str(&group, &track_id, &input.items_dir)?;
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
        )?;

        let timeout = Duration::from_secs(input.timeout_seconds);
        let reviewer =
            CodexReviewer::new(&input.model, timeout, base_prompt).with_scope_label(&group);

        let outcome =
            run_codex_review_str(&track_id, &input.items_dir, &group, &input.round_type, reviewer)?;

        outcome_to_command_outcome(outcome)
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
    pub fn review_run_claude(&self, input: ReviewRunClaudeInput) -> Result<CommandOutcome, String> {
        let track_id = resolve_track_id_or_branch_write(input.track_id, &input.items_dir)?;

        validate_track_id_str(&track_id).map_err(|e| format!("invalid --track-id: {e}"))?;
        validate_review_group_name_str(&input.group)
            .map_err(|e| format!("invalid --group: {e}"))?;

        let group = input.group.trim().to_owned();

        let maybe_briefing = get_briefing_for_scope_str(&group, &track_id, &input.items_dir)?;
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
        )?;

        let timeout = Duration::from_secs(input.timeout_seconds);
        let reviewer =
            ClaudeReviewer::new(&input.model, timeout, base_prompt).with_scope_label(&group);

        let outcome = run_claude_review_str(
            &track_id,
            &input.items_dir,
            &group,
            &input.round_type,
            reviewer,
        )?;

        outcome_to_command_outcome(outcome)
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
    pub fn review_run_local(&self, input: ReviewRunLocalInput) -> Result<CommandOutcome, String> {
        use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles, RoundType};
        use infrastructure::git_cli::{GitRepository, SystemGitRepo};

        let repo = SystemGitRepo::discover()
            .map_err(|e| format!("[ERROR] failed to discover git repository root: {e}"))?;
        let profiles_path = repo.root().join(AGENT_PROFILES_PATH);
        let profiles = AgentProfiles::load(&profiles_path)
            .map_err(|e| format!("[ERROR] failed to load agent-profiles.json: {e}"))?;

        let infra_round_type = match input.round_type.as_str() {
            "fast" => RoundType::Fast,
            "final" => RoundType::Final,
            other => {
                return Err(format!(
                    "[ERROR] unknown round type '{other}' (expected 'fast' or 'final')"
                ));
            }
        };
        let mut resolved =
            profiles.resolve_execution("reviewer", infra_round_type).ok_or_else(|| {
                "[ERROR] reviewer capability not defined in agent-profiles.json".to_owned()
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

        validate_track_id_str(&track_id).map_err(|e| format!("invalid --track-id: {e}"))?;
        validate_review_group_name_str(&group).map_err(|e| format!("invalid --group: {e}"))?;

        let maybe_briefing = get_briefing_for_scope_str(&group, &track_id, &input.items_dir)?;
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
        )?;

        let timeout = Duration::from_secs(input.timeout_seconds);

        let outcome = match resolved.provider.as_str() {
            "codex" => {
                let model = resolved.model.ok_or_else(|| {
                    "[ERROR] codex reviewer requires a model (set model in agent-profiles.json)"
                        .to_owned()
                })?;
                let reviewer =
                    CodexReviewer::new(&model, timeout, base_prompt).with_scope_label(&group);
                run_codex_review_str(
                    &track_id,
                    &input.items_dir,
                    &group,
                    &input.round_type,
                    reviewer,
                )?
            }
            "claude" => {
                let model = resolved.model.ok_or_else(|| {
                    "[ERROR] claude reviewer requires a model (set model in agent-profiles.json)"
                        .to_owned()
                })?;
                let reviewer =
                    ClaudeReviewer::new(&model, timeout, base_prompt).with_scope_label(&group);
                run_claude_review_str(
                    &track_id,
                    &input.items_dir,
                    &group,
                    &input.round_type,
                    reviewer,
                )?
            }
            other => {
                return Err(format!(
                    "[ERROR] unsupported reviewer provider '{other}' \
                     (supported: 'codex', 'claude')"
                ));
            }
        };

        outcome_to_command_outcome(outcome)
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
    ) -> Result<CommandOutcome, String> {
        use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles, RoundType};
        use infrastructure::git_cli::{GitRepository, SystemGitRepo};
        use std::sync::Arc;
        use usecase::review_v2::run_review_fix::{
            ReviewFixRunner as _, ReviewFixRunnerError, RunReviewFixCommand, RunReviewFixError,
            RunReviewFixInteractor, RunReviewFixService as _,
        };

        let repo = SystemGitRepo::discover()
            .map_err(|e| format!("[ERROR] failed to discover git repository root: {e}"))?;
        let profiles_path = repo.root().join(AGENT_PROFILES_PATH);
        let profiles = AgentProfiles::load(&profiles_path)
            .map_err(|e| format!("[ERROR] failed to load agent-profiles.json: {e}"))?;

        let track_id = input.track_id.trim().to_owned();
        validate_track_id_str(&track_id).map_err(|e| format!("invalid --track-id: {e}"))?;

        let scope = input.scope.trim().to_owned();
        validate_review_group_name_str(&scope).map_err(|e| format!("invalid --scope: {e}"))?;

        let infra_round_type = match input.round_type.as_str() {
            "fast" => RoundType::Fast,
            "final" => RoundType::Final,
            other => {
                return Err(format!(
                    "[ERROR] unknown round type '{other}' (expected 'fast' or 'final')"
                ));
            }
        };
        let resolved =
            profiles.resolve_execution("review-fix-lead", infra_round_type).ok_or_else(|| {
                "[ERROR] review-fix-lead capability not defined in agent-profiles.json".to_owned()
            })?;

        let model = resolved.model.clone().unwrap_or_else(|| input.model.clone());

        eprintln!("[sotp review fix-local] provider={} model={}", resolved.provider, &model);

        match resolved.provider.as_str() {
            "codex" => {
                let runner = CodexReviewFixRunner::new(
                    model,
                    scope.clone(),
                    input.briefing_file.clone(),
                    input.scope_files.clone(),
                );
                let runner_arc = Arc::new(runner);
                let run_fn = Arc::new(
                    move |cmd: RunReviewFixCommand| -> Result<
                        usecase::review_v2::run_review_fix::RunReviewFixOutput,
                        RunReviewFixError,
                    > {
                        runner_arc.as_ref().run_fix(cmd).map_err(|e| match e {
                            ReviewFixRunnerError::SmokeTestFailed(message) => {
                                RunReviewFixError::SmokeTestFailed(message)
                            }
                            ReviewFixRunnerError::SpawnFailed(_) => {
                                RunReviewFixError::FixRunnerFailed(
                                    "fix runner process failed".to_owned(),
                                )
                            }
                            ReviewFixRunnerError::SentinelNotFound(_) => {
                                RunReviewFixError::FixRunnerFailed(
                                    "fix runner did not report a completion status".to_owned(),
                                )
                            }
                            ReviewFixRunnerError::Unexpected(_) => {
                                RunReviewFixError::FixRunnerFailed(
                                    "fix runner failed unexpectedly".to_owned(),
                                )
                            }
                        })
                    },
                );
                let interactor = RunReviewFixInteractor::new(run_fn);
                let command = RunReviewFixCommand {
                    scope,
                    briefing_file: input.briefing_file,
                    track_id,
                    round_type: input.round_type,
                    reviewer_model: input.reviewer_model,
                    model: input.model,
                    scope_files: input.scope_files,
                };
                let output = interactor.run(command).map_err(|e| format!("[ERROR] {e}"))?;
                Ok(CommandOutcome {
                    stdout: Some(format!("REVIEW_FIX_STATUS: {}", output.status)),
                    stderr: None,
                    exit_code: u8::try_from(output.exit_code).unwrap_or(1),
                })
            }
            other => Err(format!(
                "[ERROR] unsupported review-fix-lead provider '{other}' \
                 (supported: 'codex')"
            )),
        }
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
    ) -> Result<CommandOutcome, String> {
        use usecase::review_v2::ReviewApprovalDecision;

        let track_id = resolve_track_id_or_branch(track_id, &items_dir)?;
        let output = check_approved_str(&track_id, &items_dir).map_err(|e| format!("{e}"))?;

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
    pub fn review_results(&self, input: ReviewResultsInput) -> Result<CommandOutcome, String> {
        let track_id = resolve_track_id_or_branch(input.track_id, &input.items_dir)?;

        let limit = if input.limit == 0 { None } else { Some(input.limit) };

        let output = render_review_results_str(
            &track_id,
            &input.items_dir,
            input.scope.as_deref(),
            limit,
            &input.round_type,
            input.no_hint,
        )?;

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
    ) -> Result<CommandOutcome, String> {
        use usecase::review_v2::ScopeQueryService as _;

        let track_id = resolve_track_id_or_branch(track_id, &items_dir)?;

        validate_all_paths(&paths)?;

        let interactor = build_scope_query_interactor_no_diff_str(&track_id, &items_dir)?;

        let classifications =
            interactor.classify_by_strings(paths).map_err(|e| format!("classify failed: {e}"))?;

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
    ) -> Result<CommandOutcome, String> {
        use usecase::review_v2::{ScopeQueryError, ScopeQueryService as _};

        let track_id = resolve_track_id_or_branch(track_id, &items_dir)?;

        validate_scope_for_track_str(&track_id, &items_dir, &scope)?;

        let interactor = build_scope_query_interactor_str(&track_id, &items_dir)?;
        let files = interactor.files_by_string(scope).map_err(|err| match err {
            ScopeQueryError::DiffGet(inner) => format!("diff getter failed: {inner}"),
            ScopeQueryError::UnknownScope(s) => format!("Unknown scope: {s}"),
            ScopeQueryError::InvalidPath { path, reason } => {
                format!("invalid path '{path}': {reason}")
            }
            ScopeQueryError::InvalidScopeName { name, reason } => {
                format!("invalid scope name '{name}': {reason}")
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
    ) -> Result<CommandOutcome, String> {
        let track_id = resolve_track_id_or_branch(track_id, &items_dir)?;
        validate_scope_for_track_str(&track_id, &items_dir, &scope)?;
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
    ) -> Result<CommandOutcome, String> {
        let track_id = resolve_track_id_or_branch(track_id, &items_dir)?;
        let maybe_path = get_briefing_for_scope_str(&scope, &track_id, &items_dir)?;
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
    ) -> Result<CommandOutcome, String> {
        let track_id = resolve_track_id_or_branch(track_id, &items_dir)?;
        let head_sha = persist_commit_hash_for_track(&track_id)?;
        eprintln!("[review] Recorded .commit_hash: {head_sha}");
        Ok(CommandOutcome::success(None))
    }
}

// ---------------------------------------------------------------------------
// Private helpers shared across CliApp review_v2 methods
// ---------------------------------------------------------------------------

/// Resolves a track ID: uses the provided string if `Some`, otherwise
/// resolves from the current git branch name (`track/<id>`).
///
/// # Errors
/// Returns `Err` when branch detection fails or the branch is not a track branch.
fn resolve_track_id_or_branch(
    track_id: Option<String>,
    items_dir: &std::path::Path,
) -> Result<String, String> {
    if let Some(id) = track_id {
        return Ok(id);
    }
    resolve_track_id_from_branch(items_dir)
}

/// Resolves a track ID for write operations (branch-guard variant).
///
/// When `track_id` is `Some`, validates that it matches the current branch.
/// When `None`, resolves from the current branch. Fail-closed on non-track
/// branches.
///
/// Git discovery is anchored to the repository root derived from `items_dir`
/// (stripping the trailing `track/items` segments), so that a relative
/// `items_dir` like `"track/items"` discovers the correct repo root even when
/// the process is invoked from a repo subdirectory.
///
/// # Errors
/// Returns `Err` when the explicit track ID does not match the current branch,
/// or when the current branch is not a track branch.
fn resolve_track_id_or_branch_write(
    track_id: Option<String>,
    items_dir: &std::path::Path,
) -> Result<String, String> {
    use infrastructure::git_cli::{GitRepository, SystemGitRepo};

    let project_root = crate::track::resolve_project_root(items_dir)?;
    let branch = SystemGitRepo::discover_from(&project_root)
        .and_then(|r| r.output(&["rev-parse", "--abbrev-ref", "HEAD"]))
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .map_err(|e| format!("failed to detect current branch: {e}"))?;

    let resolved_from_branch =
        branch.strip_prefix("track/").map(str::to_owned).ok_or_else(|| {
            format!(
                "current branch '{branch}' is not a track branch \
                 (expected 'track/<id>')"
            )
        })?;

    if let Some(explicit) = track_id {
        if explicit != resolved_from_branch {
            return Err(format!(
                "explicit --track-id '{explicit}' does not match current branch \
                 track '{resolved_from_branch}'. Run from the correct track branch."
            ));
        }
        return Ok(explicit);
    }

    Ok(resolved_from_branch)
}

/// Resolves the current track ID from the active git branch (`track/<id>`).
///
/// Git discovery is anchored to the repository root derived from `items_dir`
/// (stripping the trailing `track/items` segments), matching the same anchor
/// strategy used by the write-guard variant and the pre-migration resolver.
///
/// # Errors
/// Returns `Err` when git discovery fails or the branch is not a track branch.
fn resolve_track_id_from_branch(items_dir: &std::path::Path) -> Result<String, String> {
    use infrastructure::git_cli::{GitRepository, SystemGitRepo};

    let project_root = crate::track::resolve_project_root(items_dir)?;
    let output = SystemGitRepo::discover_from(&project_root)
        .and_then(|r| r.output(&["rev-parse", "--abbrev-ref", "HEAD"]))
        .map_err(|e| format!("failed to detect current branch: {e}"))?;

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    branch.strip_prefix("track/").map(str::to_owned).ok_or_else(|| {
        format!(
            "current branch '{branch}' is not a track branch \
                 (expected 'track/<id>')"
        )
    })
}

/// Builds the base prompt from an optional briefing file path or inline prompt.
///
/// # Errors
/// Returns `Err` when neither is provided or the briefing file does not exist.
fn build_base_prompt_from_input(
    briefing_file: Option<PathBuf>,
    prompt: Option<String>,
) -> Result<String, String> {
    if let Some(path) = briefing_file {
        if !path.is_file() {
            return Err(format!("briefing file not found: {}", path.display()));
        }
        Ok(format!("Read {} and perform the task described there.", path.display()))
    } else {
        prompt.ok_or_else(|| "either --briefing-file or --prompt is required".to_owned())
    }
}

/// Converts a `CodexReviewOutcome` into a `CommandOutcome`.
///
/// The verdict JSON is written to stdout; the exit code is propagated directly.
///
/// # Errors
/// Always returns `Ok` — the outcome variants only differ in exit code.
fn outcome_to_command_outcome(outcome: CodexReviewOutcome) -> Result<CommandOutcome, String> {
    match outcome {
        CodexReviewOutcome::Skipped { scope_label } => {
            eprintln!("[auto-record] Scope '{scope_label}' is empty, skipping");
            Ok(CommandOutcome {
                stdout: Some(r#"{"verdict":"zero_findings","findings":[]}"#.to_owned()),
                stderr: None,
                exit_code: 0,
            })
        }
        CodexReviewOutcome::FinalCompleted { verdict_json, exit_code } => {
            Ok(CommandOutcome { stdout: Some(verdict_json), stderr: None, exit_code })
        }
        CodexReviewOutcome::FastCompleted { verdict_json, exit_code } => {
            Ok(CommandOutcome { stdout: Some(verdict_json), stderr: None, exit_code })
        }
    }
}

/// Returns `true` if `path` is safe to inject into a reviewer prompt.
///
/// Rejects: empty strings, control characters, line separators (U+2028/U+2029),
/// backticks, absolute paths (Unix/Windows/UNC), Windows drive-letter prefixes,
/// and `..` traversal components.
fn is_safe_briefing_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    if path.chars().any(|c| c == '`' || c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')) {
        return false;
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return false;
    }
    if let (Some(first), Some(second)) = (path.as_bytes().first(), path.as_bytes().get(1)) {
        if *second == b':' && first.is_ascii_alphabetic() {
            return false;
        }
    }
    if path.split(['/', '\\']).any(|component| component == "..") {
        return false;
    }
    true
}

/// Validates all paths and returns a joined error if any fail.
///
/// Mirrors `domain::FilePath::new` validation: empty, absolute, and `..`
/// traversal paths are rejected.
///
/// # Errors
/// Returns a newline-joined string of all validation errors when any path fails.
fn validate_all_paths(paths: &[String]) -> Result<(), String> {
    let mut errors: Vec<String> = Vec::new();
    for raw in paths {
        if raw.is_empty() {
            errors.push("invalid path: empty string".to_owned());
        } else if raw.starts_with('/')
            || raw.starts_with('\\')
            || raw.get(1..3).is_some_and(|p| p == ":\\" || p == ":/")
        {
            errors.push(format!(
                "invalid path '{raw}': absolute paths are not allowed (use repo-relative)"
            ));
        } else {
            let has_traversal = raw.split(&['/', '\\'][..]).any(|seg| seg == "..");
            if has_traversal {
                errors.push(format!(
                    "invalid path '{raw}': '..' traversal components are not allowed"
                ));
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("\n")) }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    /// Serializes tests in this module that mutate the process CWD.
    /// Note: nextest runs each test in its own process, so this lock guards
    /// against races only when tests run in a shared process (e.g., `cargo test`).
    fn cwd_lock() -> &'static std::sync::Mutex<()> {
        crate::test_support::process_env_lock()
    }

    /// RAII guard: restores the process CWD to `saved` when dropped, even on panic.
    struct CwdGuard(PathBuf);
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    struct EnvGuard {
        key: &'static str,
        value: Option<OsString>,
    }
    impl EnvGuard {
        fn set(key: &'static str, value: OsString) -> Self {
            let previous = std::env::var_os(key);
            // Safety: tests that mutate process environment hold process_env_lock
            // for the full guard lifetime, so this mutation is serialized with
            // the other cwd/env-mutating tests in this crate.
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, value: previous }
        }

        fn remove(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            // Safety: see EnvGuard::set; the same process-wide lock is held while
            // removing and later restoring this variable.
            unsafe {
                std::env::remove_var(key);
            }
            Self { key, value: previous }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // Safety: see EnvGuard::set; the guard is dropped before releasing
            // the process-wide env/cwd lock.
            unsafe {
                match &self.value {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    fn run_git(root: &std::path::Path, args: &[&str]) {
        let status = Command::new("git").args(args).current_dir(root).status().unwrap();
        assert!(status.success(), "git command failed: git {}", args.join(" "));
    }

    #[cfg(unix)]
    fn make_executable(script: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(script, perms).unwrap();
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
    fn write_fake_codex_bin(bin_dir: &std::path::Path) {
        fs::create_dir_all(bin_dir).unwrap();
        let asdf = bin_dir.join("asdf");
        fs::write(&asdf, "#!/bin/sh\nexit 1\n").unwrap();
        make_executable(&asdf);

        let codex = bin_dir.join("codex");
        let script = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "codex 0.125.0"
  exit 0
fi

out=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-last-message)
      out="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

if [ -z "$out" ]; then
  echo "missing output-last-message" >&2
  exit 9
fi

cat >/dev/null
printf 'REVIEW_FIX_STATUS: completed\n' > "$out"
printf 'fake stdout\n'
exit 0
"#;
        fs::write(&codex, script).unwrap();
        make_executable(&codex);
    }

    fn run_review_fix_input(briefing_file: PathBuf) -> crate::review_v2::RunReviewFixLocalInput {
        crate::review_v2::RunReviewFixLocalInput {
            scope: "cli_composition".to_owned(),
            briefing_file: Some(briefing_file),
            track_id: "review-fix-codex-rustify-2026-05-31".to_owned(),
            round_type: "fast".to_owned(),
            reviewer_model: "gpt-5.4-mini".to_owned(),
            model: "gpt-5.5".to_owned(),
            scope_files: vec![PathBuf::from("apps/cli-composition/src/review_v2/mod.rs")],
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

        // Set up a real git repo with a track branch.
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init", "-b", "main"]);
        run_git(dir.path(), &["config", "user.email", "test@example.com"]);
        run_git(dir.path(), &["config", "user.name", "Test"]);
        fs::write(dir.path().join("README.md"), "init\n").unwrap();
        run_git(dir.path(), &["add", "README.md"]);
        run_git(dir.path(), &["commit", "-m", "init"]);
        run_git(dir.path(), &["checkout", "-b", "track/test-track"]);

        // Create `<repo>/track/items` so `resolve_project_root` finds a valid structure.
        let items_dir = dir.path().join("track/items");
        fs::create_dir_all(&items_dir).unwrap();

        // Create a subdirectory inside the repo.  From this path, the relative string
        // "track/items" does NOT point to an existing directory, so the pre-fix code
        // (`discover_from("track/items")`) would run `git -C track/items …` and fail.
        let subdir = dir.path().join("src");
        fs::create_dir_all(&subdir).unwrap();

        // Restore CWD on drop, even if an assertion panics.
        let _cwd_guard = CwdGuard(std::env::current_dir().unwrap());
        std::env::set_current_dir(&subdir).unwrap();

        // Pass the relative path — the function must succeed by anchoring to CWD (".").
        let result = super::resolve_track_id_from_branch(std::path::Path::new("track/items"));

        assert_eq!(result.unwrap(), "test-track");
    }

    #[cfg(unix)]
    #[test]
    fn review_run_fix_local_codex_completed_status_returns_command_outcome() {
        let _lock = cwd_lock().lock().unwrap();

        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init", "-b", "main"]);
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

        let _cwd_guard = CwdGuard(std::env::current_dir().unwrap());
        std::env::set_current_dir(dir.path()).unwrap();

        let outcome =
            crate::CliApp::new().review_run_fix_local(run_review_fix_input(briefing)).unwrap();

        assert_eq!(outcome.stdout.as_deref(), Some("REVIEW_FIX_STATUS: completed"));
        assert_eq!(outcome.stderr, None);
        assert_eq!(outcome.exit_code, 0);
    }

    #[test]
    fn review_run_fix_local_unsupported_provider_returns_error() {
        let _lock = cwd_lock().lock().unwrap();

        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init", "-b", "main"]);
        write_agent_profiles(dir.path(), "claude");
        let briefing = dir.path().join("briefing.md");
        fs::write(&briefing, "# Briefing\n").unwrap();

        let _cwd_guard = CwdGuard(std::env::current_dir().unwrap());
        std::env::set_current_dir(dir.path()).unwrap();

        let result = crate::CliApp::new().review_run_fix_local(run_review_fix_input(briefing));

        assert!(result.is_err(), "expected unsupported provider error, got: {result:?}");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("unsupported review-fix-lead provider 'claude'"),
            "expected unsupported provider error, got: {msg}"
        );
    }

    /// Pin that `resolve_track_id_from_branch` returns an error for a relative
    /// `items_dir` that does not follow the `*/track/items` structure.
    #[test]
    fn resolve_track_id_from_branch_rejects_non_canonical_items_dir() {
        // A path like "wrong/path" does not end in "track/items", so
        // resolve_project_root should return an error before any git I/O.
        let result = super::resolve_track_id_from_branch(std::path::Path::new("wrong/path"));
        assert!(result.is_err(), "expected error for non-canonical items_dir, got: {result:?}");
        let msg = result.unwrap_err();
        assert!(msg.contains("track/items"), "error should mention 'track/items', got: {msg}");
    }

    /// Pin the absolute path case: an absolute `items_dir` must also anchor
    /// git discovery to the derived project root, not directly to `items_dir`.
    #[test]
    fn resolve_track_id_from_branch_works_with_absolute_items_dir() {
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init", "-b", "main"]);
        run_git(dir.path(), &["config", "user.email", "test@example.com"]);
        run_git(dir.path(), &["config", "user.name", "Test"]);
        fs::write(dir.path().join("README.md"), "init\n").unwrap();
        run_git(dir.path(), &["add", "README.md"]);
        run_git(dir.path(), &["commit", "-m", "init"]);
        run_git(dir.path(), &["checkout", "-b", "track/abs-track"]);

        let items_dir = dir.path().join("track/items");
        fs::create_dir_all(&items_dir).unwrap();

        // Pass the absolute path directly — no CWD dependency.
        let result = super::resolve_track_id_from_branch(&items_dir);

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
        run_git(dir.path(), &["init", "-b", "main"]);
        run_git(dir.path(), &["config", "user.email", "test@example.com"]);
        run_git(dir.path(), &["config", "user.name", "Test"]);
        fs::write(dir.path().join("README.md"), "init\n").unwrap();
        run_git(dir.path(), &["add", "README.md"]);
        run_git(dir.path(), &["commit", "-m", "init"]);
        // Stay on `main` (not a track branch).

        let items_dir = dir.path().join("track/items");
        fs::create_dir_all(&items_dir).unwrap();

        let result = super::resolve_track_id_from_branch(&items_dir);

        assert!(result.is_err());
        let msg = result.unwrap_err();
        // The error must mention the branch name, not a git-discovery failure.
        assert!(
            msg.contains("not a track branch") || msg.contains("main"),
            "expected branch error, got: {msg}"
        );
    }

    #[test]
    fn resolve_track_id_uses_provided_id_without_git_discovery() {
        // When an explicit track ID is provided, no git I/O should occur.
        // A non-existent items_dir is fine here because track_id shortcircuits.
        let result = super::resolve_track_id_or_branch(
            Some("explicit-id".to_owned()),
            std::path::Path::new("track/items"),
        );
        assert_eq!(result.unwrap(), "explicit-id");
    }

    #[test]
    fn validate_all_paths_accepts_clean_relative_paths() {
        let result =
            super::validate_all_paths(&["src/lib.rs".to_owned(), "apps/cli/mod.rs".to_owned()]);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_all_paths_rejects_absolute_paths() {
        let result = super::validate_all_paths(&["/etc/passwd".to_owned()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("absolute paths"));
    }

    #[test]
    fn validate_all_paths_rejects_traversal_components() {
        let result = super::validate_all_paths(&["../../etc/passwd".to_owned()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("traversal"));
    }

    #[test]
    fn resolve_track_id_from_branch_returns_project_root_error_for_nested_dir() {
        // items_dir path "some/nested/dir" is not valid `*/track/items`
        let result = super::resolve_track_id_from_branch(std::path::Path::new("some/nested/dir"));
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("track/items"), "error should mention 'track/items', got: {msg}");
    }

    #[test]
    fn is_safe_briefing_path_rejects_empty() {
        assert!(!super::is_safe_briefing_path(""));
    }

    #[test]
    fn is_safe_briefing_path_rejects_absolute_unix() {
        assert!(!super::is_safe_briefing_path("/tmp/brief.md"));
    }

    #[test]
    fn is_safe_briefing_path_rejects_traversal() {
        assert!(!super::is_safe_briefing_path("../some/brief.md"));
    }

    #[test]
    fn is_safe_briefing_path_accepts_relative_clean_path() {
        assert!(super::is_safe_briefing_path("track/items/my-track/briefing.md"));
    }

    /// Confirm that `PathBuf` passed as `items_dir` is handled correctly for
    /// both read-path (explicit short-circuit) and non-canonical (error) cases,
    /// without requiring a live git repo.
    #[test]
    fn resolve_track_id_or_branch_explicit_id_bypasses_items_dir_validation() {
        // Even a clearly non-canonical items_dir is ignored when track_id is explicit.
        let result = super::resolve_track_id_or_branch(
            Some("my-track".to_owned()),
            std::path::Path::new("not/track/items"),
        );
        assert_eq!(result.unwrap(), "my-track");
    }

    #[test]
    fn resolve_track_id_or_branch_none_id_validates_items_dir_structure() {
        // When track_id is None, items_dir must follow the canonical `*/track/items` structure.
        // Use a path that genuinely does NOT end in `track/items`.
        let result =
            super::resolve_track_id_or_branch(None, std::path::Path::new("wrong/path/here"));
        assert!(result.is_err());
        let msg = result.unwrap_err();
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
        run_git(dir.path(), &["init", "-b", "main"]);
        run_git(dir.path(), &["config", "user.email", "test@example.com"]);
        run_git(dir.path(), &["config", "user.name", "Test"]);
        fs::write(dir.path().join("README.md"), "init\n").unwrap();
        run_git(dir.path(), &["add", "README.md"]);
        run_git(dir.path(), &["commit", "-m", "init"]);
        // Stay on `main` (not a track branch).

        let items_dir = dir.path().join("track/items");
        fs::create_dir_all(&items_dir).unwrap();

        let app = crate::CliApp::new();
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
        let msg = result.unwrap_err();
        // The error must be a branch error ("not a track branch", "main", or similar)
        // rather than a git-discovery error ("failed to run git", "No such file", etc.).
        assert!(
            msg.contains("not a track branch") || msg.contains("main"),
            "expected branch error, got: {msg}"
        );
    }
}
