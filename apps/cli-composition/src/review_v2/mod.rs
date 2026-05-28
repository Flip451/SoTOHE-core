//! `review_v2` command family — composition logic and CliApp impl methods.

pub mod approved;
pub mod briefing;
pub mod commit_hash;
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
pub use commit_hash::persist_commit_hash_for_track;
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
use std::time::Duration;

use infrastructure::review_v2::{ClaudeReviewer, CodexReviewer};

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
/// # Errors
/// Returns `Err` when the explicit track ID does not match the current branch,
/// or when the current branch is not a track branch.
fn resolve_track_id_or_branch_write(
    track_id: Option<String>,
    items_dir: &std::path::Path,
) -> Result<String, String> {
    use infrastructure::git_cli::{GitRepository, SystemGitRepo};

    let branch = SystemGitRepo::discover_from(items_dir)
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
/// # Errors
/// Returns `Err` when git discovery fails or the branch is not a track branch.
fn resolve_track_id_from_branch(items_dir: &std::path::Path) -> Result<String, String> {
    use infrastructure::git_cli::{GitRepository, SystemGitRepo};

    let output = SystemGitRepo::discover_from(items_dir)
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
