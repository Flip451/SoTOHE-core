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
mod pre_review_command;
pub(crate) mod run;
pub mod run_fix;
pub(crate) mod scope;
mod session_context;
pub(crate) mod shared;
mod shim;
mod telemetry_support;

pub use inputs::{ReviewRunClaudeInput, ReviewRunCodexInput, ReviewRunLocalInput};

// Public re-exports: only items consumed by external crates (e.g. apps/cli).
// All composition builders, infrastructure-typed helpers, and internal DTOs are
// pub(crate) — they do not appear on the cli_composition public face (CN-02).
pub(crate) use briefing::append_scope_briefing_reference_str;
// Demoted to pub(crate) in T010/F3: the persist helper is consumed only by
// in-crate callers (review_v2 shim + ReviewCompositionRoot + track set-commit-hash),
// so it must not appear on the cli_composition public face (CN-02 / AC-04).
pub(crate) use commit_hash::persist_commit_hash_for_track;
pub(crate) use scope::{validate_review_group_name_str, validate_track_id_str};
pub(crate) use shared::CodexReviewOutcome;

// Crate-internal helpers used only by the CliApp impl methods in this file.
use briefing::get_briefing_for_scope_str;
use helpers::{
    build_base_prompt_from_input, diagnostics_for_local_review, is_safe_briefing_path,
    outcome_to_run_review_output, resolve_track_id_or_branch, resolve_track_id_or_branch_write,
    validate_all_paths,
};
use run::{run_claude_review_str, run_codex_review_str};
use scope::validate_scope_for_track_str;
use session_context::reviewer_session_context;
use shared::{build_scope_query_interactor_no_diff_str, build_scope_query_interactor_str};
use telemetry_support::review_telemetry_for_outcome;

use std::path::PathBuf;
use std::time::Duration;

use infrastructure::agent_profiles::ResolvedExecution;
use infrastructure::review_v2::{ClaudeReviewer, CodexReviewer};
use usecase::{capability_exec::ReasoningEffort, dry_write_driver::CapabilityName};

use crate::{CommandOutcome, error::CompositionError};
use usecase::git_workflow::DiagnosticText;
use usecase::review_v2::{ReviewRunLocalOutput, RunReviewOutput};

pub use shim::ReviewCompositionRoot;

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
    pub(crate) fn review_run_codex(
        &self,
        input: ReviewRunCodexInput,
    ) -> Result<RunReviewOutput, CompositionError> {
        let track_id = resolve_track_id_or_branch_write(input.track_id, &input.items_dir)?;

        validate_track_id_str(&track_id)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid --track-id: {e}")))?;
        validate_review_group_name_str(&input.group)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid --group: {e}")))?;

        let group = input.group.trim().to_owned();

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
        let session = reviewer_session_context(
            &track_id,
            &group,
            &input.round_type,
            &input.model,
            base_prompt,
            &input.items_dir,
        )?;
        let reviewer = CodexReviewer::new(
            session.track_id,
            session.scope,
            session.round_type,
            session.diff_base,
            session.model,
            ReasoningEffort::High,
            timeout,
            session.prompt,
            session.cache,
        );

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

        outcome_to_run_review_output(run_result.map_err(CompositionError::Usecase)?)
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
    pub(crate) fn review_run_claude(
        &self,
        input: ReviewRunClaudeInput,
    ) -> Result<RunReviewOutput, CompositionError> {
        let track_id = resolve_track_id_or_branch_write(input.track_id, &input.items_dir)?;

        validate_track_id_str(&track_id)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid --track-id: {e}")))?;
        validate_review_group_name_str(&input.group)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid --group: {e}")))?;

        let group = input.group.trim().to_owned();

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
        let session = reviewer_session_context(
            &track_id,
            &group,
            &input.round_type,
            &input.model,
            base_prompt,
            &input.items_dir,
        )?;
        let reviewer = ClaudeReviewer::new(
            session.track_id,
            session.scope,
            session.round_type,
            session.diff_base,
            session.model,
            ReasoningEffort::High,
            timeout,
            session.prompt,
            session.cache,
        );

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

        outcome_to_run_review_output(run_result.map_err(CompositionError::Usecase)?)
    }

    /// Run the local reviewer with provider auto-resolved from agent-profiles.json.
    ///
    /// Resolves the `reviewer` capability from `agent-profiles.json` at the repo
    /// root, applies an optional model override, and dispatches to the appropriate
    /// reviewer implementation (codex or claude). Delegates all domain type
    /// handling to `run_codex_review_str` / `run_claude_review_str` (CN-02).
    ///
    /// Ungated execution body: callers outside the gated service graph must go
    /// through [`Self::review_run_local`] or the [`cli_driver::review::ReviewDriver`]
    /// returned by [`ReviewCompositionRoot::review_driver`].
    ///
    /// # Errors
    /// Returns `Err` when profile loading, provider resolution, arg validation,
    /// or the review cycle fails.
    pub(crate) fn review_run_local_ungated(
        &self,
        input: ReviewRunLocalInput,
    ) -> Result<ReviewRunLocalOutput, CompositionError> {
        let profiles = shared::load_agent_profiles_from_repo(Some(&input.items_dir))
            .map_err(|e| CompositionError::ConfigLoad(e.to_string()))?;
        let infra_round_type = shared::parse_round_type(&input.round_type)
            .map_err(|e| CompositionError::WiringFailed(e.to_string()))?;
        let capability = CapabilityName::try_new("reviewer")
            .map_err(|error| CompositionError::ConfigLoad(error.to_string()))?;
        let resolved = profiles
            .resolve_execution(&capability, infra_round_type)
            .map_err(|error| CompositionError::ConfigLoad(error.to_string()))?;
        let ResolvedExecution::ProviderCli { provider, model: profile_model, effort } = resolved
        else {
            return Err(CompositionError::ConfigLoad(
                "[ERROR] reviewer must resolve to a provider CLI execution".to_owned(),
            ));
        };
        let model = input.model.unwrap_or_else(|| profile_model.as_str().to_owned());

        let mut diagnostics =
            vec![format!("[sotp review local] provider={} model={}", provider, model)];

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
                diagnostics.push(format!(
                    "[WARN] briefing_file for scope '{group}' contains unsafe characters — scope-specific severity policy injection skipped"
                ));
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
        let (run_result, provider_name, effective_model) = match provider.as_str() {
            "codex" => {
                let session = reviewer_session_context(
                    &track_id,
                    &group,
                    &input.round_type,
                    &model,
                    base_prompt,
                    &input.items_dir,
                )?;
                let reviewer = CodexReviewer::new(
                    session.track_id,
                    session.scope,
                    session.round_type,
                    session.diff_base,
                    session.model,
                    effort,
                    timeout,
                    session.prompt,
                    session.cache,
                );
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
                let session = reviewer_session_context(
                    &track_id,
                    &group,
                    &input.round_type,
                    &model,
                    base_prompt,
                    &input.items_dir,
                )?;
                let reviewer = ClaudeReviewer::new(
                    session.track_id,
                    session.scope,
                    session.round_type,
                    session.diff_base,
                    session.model,
                    effort,
                    timeout,
                    session.prompt,
                    session.cache,
                );
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

        let run_outcome = run_result.map_err(CompositionError::Usecase)?;
        diagnostics.extend(diagnostics_for_local_review(&run_outcome));
        let outcome = outcome_to_run_review_output(run_outcome)?;
        Ok(ReviewRunLocalOutput {
            summary: outcome.summary,
            diagnostics: diagnostics.into_iter().map(DiagnosticText::new).collect(),
            exit_code: outcome.exit_code,
        })
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
    pub(crate) fn review_classify(
        &self,
        paths: Vec<String>,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<(String, String)>, CompositionError> {
        use usecase::review_v2::ScopeQueryService as _;

        let track_id = resolve_track_id_or_branch(track_id, &items_dir)?;

        validate_all_paths(&paths)?;

        let interactor = build_scope_query_interactor_no_diff_str(&track_id, &items_dir)
            .map_err(|e| CompositionError::WiringFailed(e.to_string()))?;

        let classifications = interactor
            .classify_by_strings(paths)
            .map_err(|e| CompositionError::Usecase(format!("classify failed: {e}")))?;

        Ok(classifications.into_iter().map(|entry| (entry.path, entry.scopes.join(","))).collect())
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
    pub(crate) fn review_files(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<String>, CompositionError> {
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

        Ok(files)
    }

    /// Validate a scope name for the given track.
    ///
    /// Resolves `track_id` from the current git branch when `None`. Returns a
    /// success `CommandOutcome` if the scope is valid, `Err` otherwise (CN-02).
    ///
    /// # Errors
    /// Returns `Err` when track ID resolution or scope validation fails.
    pub(crate) fn review_validate_scope(
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
    pub(crate) fn review_get_briefing(
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
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::{Arc, Mutex};

    use super::{ReviewCompositionRoot, ReviewRunCodexInput};
    use cli_driver::review::{
        ReviewCheckRoundSelect, ReviewCheckZeroFindingsInput, ReviewInput, ReviewResultsInput,
    };
    use domain::{
        TrackId,
        review_v2::{MainScopeName, RoundType, ScopeName},
    };
    use infrastructure::provider_session::FsProviderSessionCacheAdapter;
    use infrastructure::review_v2::ReviewCheckZeroFindingsStateAdapter;
    use usecase::review_v2::ReviewCheckZeroFindingsQuery;
    use usecase::{
        capability_exec::{ModelName, ProviderName, ReasoningEffort},
        provider_session::{
            ProviderSessionCacheEntry, ProviderSessionCacheKey, ProviderSessionCachePort,
            ProviderSessionId,
        },
    };

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
            r#"{"version":2,"groups":{"cli_composition":{"patterns":["src/**"]}},"review_operational":["track/items/<track-id>/**"]}"#,
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
        // Fail-closed per IN-06/IN-07: base_branch is read from metadata.json's
        // branch_strategy_snapshot — there is no `.harness/config/branch-strategy.json`
        // fallback, so every test track needs its own metadata.json fixture.
        fs::write(
            track_dir.join("metadata.json"),
            format!(
                r#"{{"schema_version":6,"id":"{track_id}","title":"Test Track","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","branch_strategy_snapshot":{{"base_branch":"main","merge_target":"main","merge_method":"squash"}}}}"#
            ),
        )
        .unwrap();

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
      "fast_model": "review-fast",
      "reasoning_effort": "high",
      "fast_reasoning_effort": "low",
      "execution_mode": "typed-pipeline"
    }},
    "review-fix-lead": {{
      "provider": "{provider}",
      "model": "gpt-final",
      "fast_provider": "{provider}",
      "fast_model": "gpt-fast",
      "reasoning_effort": "high",
      "fast_reasoning_effort": "low",
      "execution_mode": "typed-pipeline"
    }}
  }}
}}
"#
        );
        fs::write(config_dir.join("agent-profiles.json"), content).unwrap();
    }

    /// Profile whose `review-fix-lead` declares a fast-round override but omits
    /// `fast_reasoning_effort`, so fast-round resolution must fail closed.
    fn write_agent_profiles_missing_review_fix_fast_effort(root: &std::path::Path) {
        let config_dir = root.join(".harness/config");
        fs::create_dir_all(&config_dir).unwrap();
        let content = r#"{
  "schema_version": 1,
  "providers": {
    "codex": { "label": "Codex" }
  },
  "capabilities": {
    "review-fix-lead": {
      "provider": "codex",
      "model": "gpt-final",
      "fast_provider": "codex",
      "fast_model": "gpt-fast",
      "reasoning_effort": "high",
      "execution_mode": "typed-pipeline"
    }
  }
}
"#;
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
out=""
args="$*"
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
    fn write_recording_codex_fix_bin(bin_dir: &std::path::Path, arguments_log: &std::path::Path) {
        write_fake_codex_bin_with_body(
            bin_dir,
            &format!(
                r#"
printf '%s\n' "$args" >> '{}'
cat >/dev/null
printf 'REVIEW_FIX_STATUS: completed\n' > "$out"
exit 0
"#,
                arguments_log.display(),
            ),
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
    fn write_recording_codex_reviewer_bin(
        bin_dir: &std::path::Path,
        arguments_log: &std::path::Path,
        fail_resume: bool,
    ) {
        let resume_failure =
            if fail_resume { "case \"$args\" in *resume*) exit 7 ;; esac" } else { "" };
        write_fake_codex_bin_with_body(
            bin_dir,
            &format!(
                r#"
printf '%s' "$args" | tr '\n' ' ' >> '{}'
printf '\n' >> '{}'
{resume_failure}
printf '{{"verdict":"zero_findings","findings":[]}}\n' > "$out"
printf '{{"thread_id":"new-session"}}\n'
exit 0
"#,
                arguments_log.display(),
                arguments_log.display()
            ),
        );
    }

    #[cfg(unix)]
    fn seed_reviewer_session(
        repo: &ReviewEntrypointRepo,
        scope: ScopeName,
        round_type: RoundType,
        model: &str,
    ) {
        let cache = FsProviderSessionCacheAdapter::new(
            repo._dir.path().to_path_buf(),
            PathBuf::from("tmp/capability-runtime"),
        );
        let key = ProviderSessionCacheKey::Review {
            track_id: TrackId::try_new(repo.track_id.clone()).unwrap(),
            scope,
            round_type,
            diff_base: domain::CommitHash::try_new(
                fs::read_to_string(repo.track_dir.join(".commit_hash")).unwrap().trim(),
            )
            .unwrap(),
        };
        let entry = ProviderSessionCacheEntry::new(
            ProviderSessionId::try_new("prior-session".to_owned()).unwrap(),
            ProviderName::try_new("codex".to_owned()).unwrap(),
            ModelName::try_new(model.to_owned()).unwrap(),
            ReasoningEffort::High,
        );
        cache.save(&key, &entry).unwrap();
    }

    #[cfg(unix)]
    fn codex_review_input(repo: &ReviewEntrypointRepo, round_type: &str) -> ReviewRunCodexInput {
        ReviewRunCodexInput {
            model: "codex-review-model".to_owned(),
            timeout_seconds: 10,
            briefing_file: None,
            prompt: Some("Review.".to_owned()),
            track_id: Some(repo.track_id.clone()),
            round_type: round_type.to_owned(),
            group: "cli_composition".to_owned(),
            items_dir: repo.items_dir.clone(),
        }
    }

    fn check_zero_findings_input(repo: &ReviewEntrypointRepo) -> ReviewCheckZeroFindingsInput {
        ReviewCheckZeroFindingsInput::try_new(
            repo.items_dir.clone(),
            repo.track_id.clone(),
            "cli_composition".to_owned(),
            ReviewCheckRoundSelect::Final,
        )
        .expect("valid fixture input")
    }

    fn check_zero_findings_query(
        repo: &ReviewEntrypointRepo,
        scope: &str,
    ) -> ReviewCheckZeroFindingsQuery {
        ReviewCheckZeroFindingsQuery::try_new(
            repo.items_dir.clone(),
            repo.track_id.clone(),
            scope.to_owned(),
        )
        .expect("valid fixture query")
    }

    /// Exercises the actual composition evaluator through a persisted
    /// `review.json`, including current, stale, findings, and fast-only states.
    #[cfg(unix)]
    #[test]
    fn test_check_zero_findings_interactor_evaluates_real_review_json_states() {
        use domain::review_v2::{FastVerdict, ReviewHash, ReviewWriter, ReviewerFinding, Verdict};
        use infrastructure::review_v2::FsReviewStore;
        use usecase::review_v2::{
            ReviewCheckZeroFindingsInteractor, ReviewCheckZeroFindingsOutcome,
            ReviewCheckZeroFindingsService as _,
        };

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("check-zero-findings-2026");
        let bin_dir = repo.track_dir.join("fake-bin-check-zero-findings");
        write_fake_codex_reviewer_bin(&bin_dir);
        let _path_guard = prepend_path(&bin_dir);
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        let review_scope = ScopeName::Main(MainScopeName::new("cli_composition").unwrap());
        let query = check_zero_findings_query(&repo, "cli_composition");
        let interactor = ReviewCheckZeroFindingsInteractor::new(std::sync::Arc::new(
            ReviewCheckZeroFindingsStateAdapter,
        ));
        let driver = ReviewCompositionRoot::new().review_driver();

        ReviewCompositionRoot::new().review_run_codex(codex_review_input(&repo, "final")).unwrap();

        let foreign_repo = tempfile::tempdir().unwrap();
        GitRunner::at(foreign_repo.path()).assert_success(&["init", "-b", "main"]);
        std::env::set_current_dir(foreign_repo.path()).unwrap();
        let cwd_before_check = std::env::current_dir().unwrap();
        let check_outcome =
            driver.handle(ReviewInput::CheckZeroFindings(check_zero_findings_input(&repo)));
        assert_eq!(check_outcome.exit_code, 0, "{check_outcome:?}");
        assert_eq!(std::env::current_dir().unwrap(), cwd_before_check);

        fs::write(repo._dir.path().join("src/lib.rs"), "pub fn changed_again() {}\n").unwrap();
        assert_eq!(
            interactor.check_zero_findings(&query).unwrap(),
            ReviewCheckZeroFindingsOutcome::StaleFinalVerdict
        );

        let store = FsReviewStore::new(repo.track_dir.join("review.json"), repo.track_dir.clone());
        let finding = ReviewerFinding::new("remaining finding", None, None, None, None).unwrap();
        let findings = Verdict::findings_remain(vec![finding]).unwrap();
        let persisted_hash = ReviewHash::computed("rvw1:sha256:abcdef0123456789").unwrap();
        store.write_verdict(&review_scope, &findings, &persisted_hash).unwrap();
        assert_eq!(
            interactor.check_zero_findings(&query).unwrap(),
            ReviewCheckZeroFindingsOutcome::FindingsRemain
        );

        store.reset().unwrap();
        store
            .write_fast_verdict(&review_scope, &FastVerdict::ZeroFindings, &persisted_hash)
            .unwrap();
        assert_eq!(
            interactor.check_zero_findings(&query).unwrap(),
            ReviewCheckZeroFindingsOutcome::MissingFinalVerdict
        );

        let unknown_scope = check_zero_findings_query(&repo, "not-configured");
        assert_eq!(
            interactor.check_zero_findings(&unknown_scope).unwrap(),
            ReviewCheckZeroFindingsOutcome::MissingFinalVerdict
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_review_driver_check_zero_findings_current_fast_only_verdict_returns_nonzero_exit() {
        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("check-zero-findings-fast-only-2026");
        let bin_dir = repo.track_dir.join("fake-bin-check-zero-findings-fast-only");
        write_fake_codex_reviewer_bin(&bin_dir);
        let _path_guard = prepend_path(&bin_dir);
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        ReviewCompositionRoot::new().review_run_codex(codex_review_input(&repo, "fast")).unwrap();

        let outcome = ReviewCompositionRoot::new()
            .review_driver()
            .handle(ReviewInput::CheckZeroFindings(check_zero_findings_input(&repo)));

        assert_ne!(outcome.exit_code, 0, "a current fast-only verdict must fail closed");
        assert!(outcome.stderr.as_deref().is_some_and(|message| message.contains("final")));
    }

    #[cfg(unix)]
    #[test]
    fn test_review_driver_check_zero_findings_uses_isolated_git_for_base_fallback() {
        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("check-zero-findings-git-dir-2026");
        let bin_dir = repo.track_dir.join("fake-bin-check-zero-findings-git-dir");
        write_fake_codex_reviewer_bin(&bin_dir);
        let _path_guard = prepend_path(&bin_dir);
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        ReviewCompositionRoot::new().review_run_codex(codex_review_input(&repo, "final")).unwrap();
        fs::remove_file(repo.track_dir.join(".commit_hash")).unwrap();

        let foreign_repo = tempfile::tempdir().unwrap();
        GitRunner::at(foreign_repo.path()).assert_success(&["init", "-b", "main"]);
        let _git_dir_guard = EnvGuard::set("GIT_DIR", foreign_repo.path().join(".git"));

        let outcome = ReviewCompositionRoot::new()
            .review_driver()
            .handle(ReviewInput::CheckZeroFindings(check_zero_findings_input(&repo)));

        assert_eq!(
            outcome.exit_code, 0,
            "GIT_DIR must not redirect branch-fallback base resolution"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_review_driver_check_zero_findings_uses_isolated_git_for_commit_hash_ancestry() {
        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("check-zero-findings-ancestry-git-dir-2026");
        let bin_dir = repo.track_dir.join("fake-bin-check-zero-findings-ancestry-git-dir");
        write_fake_codex_reviewer_bin(&bin_dir);
        let _path_guard = prepend_path(&bin_dir);
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        ReviewCompositionRoot::new().review_run_codex(codex_review_input(&repo, "final")).unwrap();

        let metadata_path = repo.track_dir.join("metadata.json");
        let metadata = fs::read_to_string(&metadata_path).unwrap();
        let invalid_fallback_metadata =
            metadata.replace("\"base_branch\":\"main\"", "\"base_branch\":\"missing-base\"");
        assert_ne!(invalid_fallback_metadata, metadata, "fixture must invalidate base fallback");
        fs::write(&metadata_path, invalid_fallback_metadata).unwrap();

        let foreign_repo = tempfile::tempdir().unwrap();
        GitRunner::at(foreign_repo.path()).assert_success(&["init", "-b", "main"]);
        let _git_dir_guard = EnvGuard::set("GIT_DIR", foreign_repo.path().join(".git"));

        let outcome = ReviewCompositionRoot::new()
            .review_driver()
            .handle(ReviewInput::CheckZeroFindings(check_zero_findings_input(&repo)));

        assert_eq!(
            outcome.exit_code, 0,
            "GIT_DIR must not redirect persisted commit-hash ancestry validation"
        );
    }

    #[test]
    fn test_check_zero_findings_state_adapter_rejects_revision_expression_base_branch() {
        use usecase::review_v2::ReviewCheckZeroFindingsStatePort as _;

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("check-zero-findings-invalid-base-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();
        fs::remove_file(repo.track_dir.join(".commit_hash")).unwrap();
        let metadata_path = repo.track_dir.join("metadata.json");
        let metadata = fs::read_to_string(&metadata_path).unwrap();
        let invalid_metadata =
            metadata.replace("\"base_branch\":\"main\"", "\"base_branch\":\"HEAD~1\"");
        assert_ne!(invalid_metadata, metadata, "fixture must set a revision expression");
        fs::write(&metadata_path, invalid_metadata).unwrap();

        let result = ReviewCheckZeroFindingsStateAdapter.state_for(
            &TrackId::try_new(repo.track_id).unwrap(),
            &repo.items_dir,
            &ScopeName::Main(MainScopeName::new("cli_composition").unwrap()),
        );

        assert!(result.is_err(), "revision expressions must not resolve as base branches");
    }

    #[test]
    fn test_check_zero_findings_interactor_corrupt_review_json_returns_evaluation_failed() {
        use usecase::review_v2::{
            ReviewCheckZeroFindingsEvaluationError, ReviewCheckZeroFindingsInteractor,
            ReviewCheckZeroFindingsService as _,
        };

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("check-zero-findings-corrupt-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();
        fs::write(repo.track_dir.join("review.json"), "{not valid json").unwrap();

        let query = check_zero_findings_query(&repo, "cli_composition");
        let interactor = ReviewCheckZeroFindingsInteractor::new(std::sync::Arc::new(
            ReviewCheckZeroFindingsStateAdapter,
        ));

        assert!(matches!(
            interactor.check_zero_findings(&query),
            Err(ReviewCheckZeroFindingsEvaluationError::EvaluationFailed(_))
        ));
    }

    #[test]
    fn test_check_zero_findings_interactor_absent_review_json_returns_missing_final_verdict() {
        use usecase::review_v2::{
            ReviewCheckZeroFindingsInteractor, ReviewCheckZeroFindingsOutcome,
            ReviewCheckZeroFindingsService as _,
        };

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("check-zero-findings-absent-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();
        assert!(!repo.track_dir.join("review.json").exists());

        let query = check_zero_findings_query(&repo, "cli_composition");
        let interactor = ReviewCheckZeroFindingsInteractor::new(std::sync::Arc::new(
            ReviewCheckZeroFindingsStateAdapter,
        ));

        assert_eq!(
            interactor.check_zero_findings(&query).unwrap(),
            ReviewCheckZeroFindingsOutcome::MissingFinalVerdict
        );
    }

    #[test]
    fn test_check_zero_findings_interactor_empty_scope_returns_missing_final_verdict() {
        use usecase::review_v2::{
            ReviewCheckZeroFindingsInteractor, ReviewCheckZeroFindingsOutcome,
            ReviewCheckZeroFindingsService as _,
        };

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("check-zero-findings-empty-scope-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();
        fs::write(
            repo._dir.path().join(".harness/config/review-scope.json"),
            r#"{"version":2,"groups":{"cli_composition":{"patterns":["src/**"]},"empty_scope":{"patterns":["empty/**"]}}}"#,
        )
        .unwrap();

        let query = check_zero_findings_query(&repo, "empty_scope");
        let interactor = ReviewCheckZeroFindingsInteractor::new(std::sync::Arc::new(
            ReviewCheckZeroFindingsStateAdapter,
        ));

        assert_eq!(
            interactor.check_zero_findings(&query).unwrap(),
            ReviewCheckZeroFindingsOutcome::MissingFinalVerdict
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_check_zero_findings_state_adapter_rejects_symlinked_items_dir() {
        use usecase::review_v2::ReviewCheckZeroFindingsStatePort as _;

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("check-zero-findings-symlink-items-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();
        let symlinked_items_dir = repo._dir.path().join("symlinked-items");
        std::os::unix::fs::symlink(&repo.items_dir, &symlinked_items_dir).unwrap();
        let track_id = TrackId::try_new(repo.track_id).unwrap();
        let scope = ScopeName::Main(MainScopeName::new("cli_composition").unwrap());

        let result =
            ReviewCheckZeroFindingsStateAdapter.state_for(&track_id, &symlinked_items_dir, &scope);

        assert!(result.is_err(), "symlinked items_dir must fail closed: {result:?}");
    }

    #[test]
    fn test_check_zero_findings_state_adapter_rejects_malformed_commit_hash() {
        use usecase::review_v2::ReviewCheckZeroFindingsStatePort as _;

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("check-zero-findings-malformed-hash-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();
        fs::write(repo.track_dir.join(".commit_hash"), "not-a-commit-hash\n").unwrap();
        let track_id = TrackId::try_new(repo.track_id).unwrap();
        let scope = ScopeName::Main(MainScopeName::new("cli_composition").unwrap());

        let result =
            ReviewCheckZeroFindingsStateAdapter.state_for(&track_id, &repo.items_dir, &scope);

        assert!(result.is_err(), "malformed .commit_hash must fail closed: {result:?}");
    }

    #[test]
    fn test_check_zero_findings_state_adapter_uses_base_branch_when_commit_hash_absent() {
        use domain::review_v2::{RequiredReason, ReviewState};
        use usecase::review_v2::ReviewCheckZeroFindingsStatePort as _;

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("check-zero-findings-absent-hash-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();
        fs::remove_file(repo.track_dir.join(".commit_hash")).unwrap();
        let track_id = TrackId::try_new(repo.track_id).unwrap();
        let scope = ScopeName::Main(MainScopeName::new("cli_composition").unwrap());

        let state = ReviewCheckZeroFindingsStateAdapter
            .state_for(&track_id, &repo.items_dir, &scope)
            .unwrap();

        assert_eq!(state, Some(ReviewState::Required(RequiredReason::NotStarted)));
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

    fn run_review_fix_input(
        briefing_file: PathBuf,
        round_type: usecase::review_v2::ReviewRoundType,
    ) -> cli_driver::review::ReviewFixInput {
        cli_driver::review::ReviewFixInput::new(
            "cli_composition".to_owned(),
            briefing_file,
            Some("review-fix-codex-rustify-2026-05-31".to_owned()),
            PathBuf::from("track/items"),
            match round_type {
                usecase::review_v2::ReviewRoundType::Fast => "fast".to_owned(),
                usecase::review_v2::ReviewRoundType::Final => "final".to_owned(),
            },
            Some("gpt-5.5".to_owned()),
        )
    }

    fn activate_review_fix_track(dir: &Path) {
        fs::write(dir.join("README.md"), "review-fix fixture\n").unwrap();
        fs::create_dir_all(dir.join("track/items")).unwrap();
        GitRunner::at(dir).assert_success(&["config", "user.email", "test@example.invalid"]);
        GitRunner::at(dir).assert_success(&["config", "user.name", "Test User"]);
        GitRunner::at(dir).assert_success(&["add", "README.md"]);
        GitRunner::at(dir).assert_success(&["commit", "-m", "fixture"]);
        GitRunner::at(dir).assert_success(&[
            "checkout",
            "-b",
            "track/review-fix-codex-rustify-2026-05-31",
        ]);
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
        assert!(outcome.summary.as_deref().unwrap_or("").contains("zero_findings"));
        assert!(repo.track_dir.join("review.json").exists());
        assert_review_telemetry(&repo.track_dir, "codex", "codex-review-model", "codex", "fast");
    }

    #[cfg(unix)]
    #[test]
    fn test_review_composition_root_resumes_matching_scope_session_and_persists_review_record() {
        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-session-resume-2026");
        let bin_dir = repo.track_dir.join("fake-bin-session-resume");
        let arguments_log = repo.track_dir.join("codex-arguments.log");
        write_recording_codex_reviewer_bin(&bin_dir, &arguments_log, false);
        seed_reviewer_session(
            &repo,
            ScopeName::Main(MainScopeName::new("cli_composition").unwrap()),
            RoundType::Fast,
            "codex-review-model",
        );
        let _path_guard = prepend_path(&bin_dir);
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        let outcome = ReviewCompositionRoot::new()
            .review_run_codex(codex_review_input(&repo, "fast"))
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        let arguments = fs::read_to_string(arguments_log).unwrap();
        assert!(arguments.contains("resume prior-session"), "expected resume argv: {arguments}");
        assert!(
            arguments.contains("--model codex-review-model"),
            "missing model argv: {arguments}"
        );
        assert!(arguments.contains("--sandbox read-only"), "missing sandbox argv: {arguments}");
        assert!(
            arguments.contains("model_reasoning_effort=\"high\""),
            "missing effort argv: {arguments}"
        );
        let review_record = fs::read_to_string(repo.track_dir.join("review.json")).unwrap();
        assert!(review_record.contains("zero_findings"));
        assert!(review_record.contains("fast"));
    }

    #[cfg(unix)]
    #[test]
    fn test_review_composition_root_starts_fresh_for_model_mismatch_first_round_and_fast_to_final()
    {
        let _lock = cwd_lock().lock().unwrap();
        for (name, round_type, seeded) in [
            ("model-mismatch", "fast", Some((RoundType::Fast, "previous-model"))),
            ("first-round", "fast", None),
            ("fast-to-final", "final", Some((RoundType::Fast, "codex-review-model"))),
        ] {
            let repo = setup_review_entrypoint_repo(&format!("review-session-{name}-2026"));
            let bin_dir = repo.track_dir.join("fake-bin-session-fresh");
            let arguments_log = repo.track_dir.join("codex-arguments.log");
            write_recording_codex_reviewer_bin(&bin_dir, &arguments_log, false);
            if let Some((seed_round, model)) = seeded {
                seed_reviewer_session(
                    &repo,
                    ScopeName::Main(MainScopeName::new("cli_composition").unwrap()),
                    seed_round,
                    model,
                );
            }
            let _path_guard = prepend_path(&bin_dir);
            let _cwd_guard = CwdGuard::save_current();
            std::env::set_current_dir(repo._dir.path()).unwrap();

            let outcome = ReviewCompositionRoot::new()
                .review_run_codex(codex_review_input(&repo, round_type))
                .unwrap();

            assert_eq!(outcome.exit_code, 0, "{name}");
            let arguments = fs::read_to_string(arguments_log).unwrap();
            assert!(!arguments.contains("resume"), "{name} must start fresh: {arguments}");
            assert!(arguments.contains("--model codex-review-model"), "{name}: {arguments}");
            assert!(arguments.contains("--sandbox read-only"), "{name}: {arguments}");
            assert!(arguments.contains("model_reasoning_effort=\"high\""), "{name}: {arguments}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_review_composition_root_resume_failure_falls_back_to_fresh_session() {
        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-session-fallback-2026");
        let bin_dir = repo.track_dir.join("fake-bin-session-fallback");
        let arguments_log = repo.track_dir.join("codex-arguments.log");
        write_recording_codex_reviewer_bin(&bin_dir, &arguments_log, true);
        seed_reviewer_session(
            &repo,
            ScopeName::Main(MainScopeName::new("cli_composition").unwrap()),
            RoundType::Fast,
            "codex-review-model",
        );
        let _path_guard = prepend_path(&bin_dir);
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        let outcome = ReviewCompositionRoot::new()
            .review_run_codex(codex_review_input(&repo, "fast"))
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        let arguments = fs::read_to_string(arguments_log).unwrap();
        let attempts: Vec<&str> = arguments.lines().collect();
        assert_eq!(attempts.len(), 2, "resume failure must retry fresh: {attempts:?}");
        assert!(attempts.first().is_some_and(|attempt| attempt.contains("resume prior-session")));
        assert!(attempts.get(1).is_some_and(|attempt| !attempt.contains("resume")));
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
        assert!(outcome.summary.as_deref().unwrap_or("").contains("zero_findings"));
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
            .review_run_local_ungated(crate::review_v2::ReviewRunLocalInput {
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
        assert!(outcome.summary.as_deref().unwrap_or("").contains("zero_findings"));
        assert!(repo.track_dir.join("review.json").exists());
        assert_review_telemetry(&repo.track_dir, "claude", "review-fast", "claude", "fast");
    }

    #[cfg(unix)]
    #[test]
    fn review_fix_driver_codex_completed_status_returns_command_outcome() {
        let _lock = cwd_lock().lock().unwrap();

        let dir = tempfile::tempdir().unwrap();
        GitRunner::at(dir.path()).assert_success(&["init", "-b", "main"]);
        activate_review_fix_track(dir.path());
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

        let output = crate::review_v2::ReviewCompositionRoot::new().review_fix_driver().handle(
            run_review_fix_input(
                PathBuf::from("briefing.md"),
                usecase::review_v2::ReviewRoundType::Fast,
            ),
        );

        assert_eq!(
            output.stdout.as_deref(),
            Some("REVIEW_FIX_STATUS: completed"),
            "review-fix outcome: {output:?}"
        );
        assert_eq!(output.stderr, None);
        assert_eq!(output.exit_code, 0);
    }

    #[cfg(unix)]
    #[test]
    fn test_review_fix_driver_injects_resolved_effort_for_fast_and_final_rounds() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        GitRunner::at(dir.path()).assert_success(&["init", "-b", "main"]);
        activate_review_fix_track(dir.path());
        write_agent_profiles(dir.path(), "codex");
        let briefing = dir.path().join("briefing.md");
        fs::write(&briefing, "# Briefing\n").unwrap();
        let bin_dir = dir.path().join("bin-test");
        let arguments_log = dir.path().join("codex-arguments.log");
        write_recording_codex_fix_bin(&bin_dir, &arguments_log);
        let _path_guard = prepend_path(&bin_dir);
        let _sandbox_guard = EnvGuard::remove("CODEX_SANDBOX");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(dir.path()).unwrap();

        for (round_type, expected_effort) in [("fast", "low"), ("final", "high")] {
            let command = run_review_fix_input(
                PathBuf::from("briefing.md"),
                if round_type == "fast" {
                    usecase::review_v2::ReviewRoundType::Fast
                } else {
                    usecase::review_v2::ReviewRoundType::Final
                },
            );
            let output =
                crate::review_v2::ReviewCompositionRoot::new().review_fix_driver().handle(command);
            assert_eq!(output.exit_code, 0, "{round_type} round must complete: {output:?}");
            let arguments = fs::read_to_string(&arguments_log).unwrap();
            let invocation = arguments.lines().last().unwrap_or_default();
            assert!(
                invocation.contains(&format!("model_reasoning_effort=\"{expected_effort}\"")),
                "{round_type} effort must be explicit in fixer argv: {invocation}"
            );
        }
    }

    #[test]
    fn review_fix_driver_claude_provider_renders_subagent_dispatch_instruction() {
        // Composition injects the typed dispatch path; only the CLI driver
        // invokes it and renders the sentinel and JSON protocol.
        let _lock = cwd_lock().lock().unwrap();

        let dir = tempfile::tempdir().unwrap();
        GitRunner::at(dir.path()).assert_success(&["init", "-b", "main"]);
        activate_review_fix_track(dir.path());
        write_agent_profiles(dir.path(), "claude");
        let briefing = dir.path().join("briefing.md");
        fs::write(&briefing, "# Briefing\n").unwrap();

        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(dir.path()).unwrap();

        let outcome = crate::review_v2::ReviewCompositionRoot::new().review_fix_driver().handle(
            run_review_fix_input(
                PathBuf::from("briefing.md"),
                usecase::review_v2::ReviewRoundType::Fast,
            ),
        );

        assert_eq!(
            outcome.exit_code,
            cli_driver::review::SUBAGENT_DISPATCH_EXIT_CODE,
            "review-fix outcome: {outcome:?}"
        );
        let stdout = outcome.stdout.unwrap();
        assert!(stdout.starts_with(cli_driver::review::SUBAGENT_DISPATCH_SENTINEL));
        assert!(stdout.contains("\"agent\":\"review-fix-lead\""));
        assert!(stdout.contains("\"model\":\"gpt-5.5\""));
        assert!(stdout.contains("\"effort\":\"low\""));
        assert!(stdout.contains("\"scope\":\"cli_composition\""));
        assert!(stdout.contains("\"briefing_file\":\"briefing.md\""));
        assert!(stdout.contains("\"track_id\":\"review-fix-codex-rustify-2026-05-31\""));
        assert!(stdout.contains(&format!("\"repository_root\":\"{}\"", dir.path().display())));
        assert!(stdout.contains("\"round_type\":\"fast\""));
    }

    #[test]
    fn test_review_fix_driver_real_runner_adapter_delivers_raw_input_to_claude_dispatch() {
        let _lock = cwd_lock().lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        GitRunner::at(directory.path()).assert_success(&["init", "-b", "main"]);
        activate_review_fix_track(directory.path());
        write_agent_profiles(directory.path(), "claude");
        let briefing = directory.path().join("briefing.md");
        fs::write(&briefing, "# Briefing\n").unwrap();

        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(directory.path()).unwrap();

        let outcome = crate::review_v2::ReviewCompositionRoot::new().review_fix_driver().handle(
            run_review_fix_input(
                PathBuf::from("briefing.md"),
                usecase::review_v2::ReviewRoundType::Fast,
            ),
        );

        assert_eq!(outcome.exit_code, cli_driver::review::SUBAGENT_DISPATCH_EXIT_CODE);
        assert_eq!(outcome.stderr, None);
        let stdout = outcome.stdout.expect("the driver must render a dispatch outcome");
        assert!(stdout.starts_with(cli_driver::review::SUBAGENT_DISPATCH_SENTINEL));
        assert!(stdout.contains("\"agent\":\"review-fix-lead\""));
        assert!(stdout.contains("\"briefing_file\":\"briefing.md\""));
        assert!(
            stdout.contains(&format!("\"repository_root\":\"{}\"", directory.path().display()))
        );
    }

    #[test]
    fn test_review_fix_driver_delivers_briefing_path_to_runner() {
        struct CapturingRunner {
            command: Arc<Mutex<Option<usecase::review_v2::run_review_fix::RunReviewFixCommand>>>,
        }

        impl usecase::review_v2::run_review_fix::ReviewFixRunner for CapturingRunner {
            fn run_fix(
                &self,
                command: usecase::review_v2::run_review_fix::RunReviewFixCommand,
            ) -> Result<
                usecase::review_v2::run_review_fix::RunReviewFixOutput,
                usecase::review_v2::run_review_fix::ReviewFixRunnerError,
            > {
                *self.command.lock().unwrap() = Some(command);
                Ok(usecase::review_v2::run_review_fix::RunReviewFixOutput {
                    status: "completed".to_owned(),
                    exit_code: 0,
                    stderr: None,
                })
            }
        }

        let _lock = cwd_lock().lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        GitRunner::at(directory.path()).assert_success(&["init", "-b", "main"]);
        activate_review_fix_track(directory.path());
        fs::write(directory.path().join("briefing.md"), "# Configured briefing\n").unwrap();

        let captured = Arc::new(Mutex::new(None));
        let service =
            super::run_fix::review_fix_service_with_capturing_runner(Arc::new(CapturingRunner {
                command: Arc::clone(&captured),
            }));
        let driver = cli_driver::review::ReviewFixDriver::new(service);

        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(directory.path()).unwrap();
        let outcome = driver.handle(run_review_fix_input(
            PathBuf::from("briefing.md"),
            usecase::review_v2::ReviewRoundType::Fast,
        ));

        assert_eq!(outcome.exit_code, 0, "review-fix outcome: {outcome:?}");
        let command =
            captured.lock().unwrap().take().expect("the path-delivery flow must invoke the runner");
        assert_eq!(command.briefing_file(), Path::new("briefing.md"));
    }

    #[test]
    fn test_review_run_fix_local_rejects_missing_effort_before_dispatch() {
        // CN-01: a review-fix dispatch whose resolved round has no configured
        // effort must be refused fail-closed (no fall-through to the provider
        // default). The rejection surfaces before any runner subprocess is
        // constructed, as a typed `RunReviewFixError`.
        let _lock = cwd_lock().lock().unwrap();

        let dir = tempfile::tempdir().unwrap();
        GitRunner::at(dir.path()).assert_success(&["init", "-b", "main"]);
        activate_review_fix_track(dir.path());
        write_agent_profiles_missing_review_fix_fast_effort(dir.path());
        let briefing = dir.path().join("briefing.md");
        fs::write(&briefing, "# Briefing\n").unwrap();

        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(dir.path()).unwrap();

        let outcome = crate::review_v2::ReviewCompositionRoot::new().review_fix_driver().handle(
            run_review_fix_input(
                PathBuf::from("briefing.md"),
                usecase::review_v2::ReviewRoundType::Fast,
            ),
        );

        assert_ne!(outcome.exit_code, 0, "effort-less fast round must be rejected");
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|message| message.contains("no reasoning effort")),
            "rejection must cite the missing effort, got: {outcome:?}"
        );
    }

    #[test]
    fn test_review_service_validate_scope_accepts_known_and_rejects_unknown_scope() {
        use usecase::review_v2::aggregate_service::ReviewService as _;

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-service-aux-scope-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        let service = super::shim::review_service_impl();

        service
            .validate_scope(
                "cli_composition".to_owned(),
                Some(repo.track_id.clone()),
                repo.items_dir.clone(),
            )
            .expect("configured scope must validate");

        let error = service
            .validate_scope(
                "no-such-scope".to_owned(),
                Some(repo.track_id.clone()),
                repo.items_dir.clone(),
            )
            .expect_err("unconfigured scope must be rejected");
        assert!(
            error.to_string().contains("Unknown scope"),
            "rejection must name the unknown scope, got: {error}"
        );
    }

    #[test]
    fn test_review_service_get_briefing_returns_none_without_config_and_rejects_bad_scope() {
        use usecase::review_v2::aggregate_service::ReviewService as _;

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-service-aux-briefing-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        let service = super::shim::review_service_impl();

        let briefing = service
            .get_briefing(
                "cli_composition".to_owned(),
                Some(repo.track_id.clone()),
                repo.items_dir.clone(),
            )
            .expect("configured scope without a briefing entry must succeed");
        assert_eq!(briefing, None, "fixture config declares no briefing file");

        let error = service
            .get_briefing(
                "cli_composition".to_owned(),
                Some("Invalid Track Id!".to_owned()),
                repo.items_dir.clone(),
            )
            .expect_err("invalid track id must be rejected");
        assert!(
            error.to_string().contains("invalid --track-id"),
            "rejection must cite the invalid track id, got: {error}"
        );
    }

    #[test]
    fn test_review_service_check_approved_preserves_decision_and_exit_contract() {
        use usecase::review_v2::{ReviewApprovalDecision, aggregate_service::ReviewService as _};

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-service-approved-contract-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();
        let service = super::shim::review_service_impl();
        let driver = ReviewCompositionRoot::new().review_driver();

        GitRunner::at(repo._dir.path()).assert_success(&["checkout", "main"]);
        fs::write(
            repo.track_dir.join(".commit_hash"),
            git_stdout(repo._dir.path(), &["rev-parse", "HEAD"]),
        )
        .unwrap();
        let approved = service
            .check_approved(repo.track_id.clone(), repo.items_dir.clone())
            .expect("current commit with no changed scopes must be approved");
        assert_eq!(approved.decision, ReviewApprovalDecision::Approved);
        assert_eq!(approved.bypass_scope_count, None);
        assert!(approved.blocked_scopes.is_empty());
        let approved_outcome = driver
            .handle(ReviewInput::CheckApproved(repo.track_id.clone(), repo.items_dir.clone()));
        assert_eq!(approved_outcome.exit_code, 0, "{approved_outcome:?}");
        assert!(approved_outcome.stderr.as_deref().is_some_and(|message| message.contains("[OK]")));

        GitRunner::at(repo._dir.path())
            .assert_success(&["checkout", &format!("track/{}", repo.track_id)]);
        fs::write(
            repo.track_dir.join(".commit_hash"),
            git_stdout(repo._dir.path(), &["rev-parse", "HEAD~1"]),
        )
        .unwrap();
        let bypassed = service
            .check_approved(repo.track_id.clone(), repo.items_dir.clone())
            .expect("missing review state must use the approval bypass");
        assert_eq!(bypassed.decision, ReviewApprovalDecision::ApprovedWithBypass);
        assert_eq!(bypassed.bypass_scope_count, Some(1));
        assert!(bypassed.blocked_scopes.is_empty());
        let bypass_outcome = driver
            .handle(ReviewInput::CheckApproved(repo.track_id.clone(), repo.items_dir.clone()));
        assert_eq!(bypass_outcome.exit_code, 0, "{bypass_outcome:?}");
        assert!(bypass_outcome
            .stderr
            .as_deref()
            .is_some_and(|message| message.contains("[WARN]") && message.contains("1 scope(s)")));

        fs::write(repo.track_dir.join("review.json"), r#"{"schema_version":2,"scopes":{}}"#)
            .unwrap();
        let blocked = service
            .check_approved(repo.track_id.clone(), repo.items_dir.clone())
            .expect("missing required review scope must yield a blocked DTO");
        assert_eq!(blocked.decision, ReviewApprovalDecision::Blocked);
        assert_eq!(blocked.bypass_scope_count, None);
        assert_eq!(blocked.blocked_scopes, vec!["cli_composition"]);
        let blocked_outcome = driver
            .handle(ReviewInput::CheckApproved(repo.track_id.clone(), repo.items_dir.clone()));
        assert_ne!(blocked_outcome.exit_code, 0, "{blocked_outcome:?}");
        assert!(blocked_outcome.stderr.as_deref().is_some_and(|message| {
            message.contains("[BLOCKED]") && message.contains("cli_composition")
        }));
    }

    #[test]
    fn test_review_service_provider_methods_reject_invalid_track_before_dispatch() {
        use usecase::review_v2::aggregate_service::ReviewService as _;

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-service-provider-2026");
        write_agent_profiles(repo._dir.path(), "codex");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();
        let service = super::shim::review_service_impl();
        let input = || usecase::review_v2::ReviewRunInput {
            model: "test-model".to_owned(),
            timeout_seconds: 1,
            briefing_file: None,
            prompt: None,
            track_id: Some("Invalid Track Id!".to_owned()),
            round_type: "fast".to_owned(),
            group: "cli_composition".to_owned(),
            items_dir: repo.items_dir.clone(),
        };

        assert!(service.run_codex(input()).is_err());
        assert!(service.run_claude(input()).is_err());

        let local = service.run_local(
            None,
            1,
            None,
            None,
            Some("Invalid Track Id!".to_owned()),
            "fast".to_owned(),
            "cli_composition".to_owned(),
            repo.items_dir.clone(),
        );
        assert_eq!(local.exit_code, 1);
        assert!(
            !local.diagnostics.is_empty(),
            "local review must expose the rejected input as a diagnostic"
        );
    }

    #[test]
    fn test_review_service_auxiliary_methods_reject_invalid_track() {
        use usecase::review_v2::aggregate_service::ReviewService as _;

        let repo = setup_review_entrypoint_repo("review-service-auxiliary-2026");
        let service = super::shim::review_service_impl();
        let invalid_track = Some("Invalid Track Id!".to_owned());

        assert!(
            service.check_approved("Invalid Track Id!".to_owned(), repo.items_dir.clone()).is_err()
        );
        assert!(
            service
                .classify(
                    vec!["src/lib.rs".to_owned()],
                    invalid_track.clone(),
                    repo.items_dir.clone(),
                )
                .is_err()
        );
        assert!(
            service.files("cli_composition".to_owned(), invalid_track, repo.items_dir,).is_err()
        );
    }

    #[test]
    fn test_review_service_classify_and_files_return_happy_outputs() {
        use usecase::review_v2::aggregate_service::ReviewService as _;

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-service-auxiliary-happy-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();
        let service = super::shim::review_service_impl();

        let classifications = service
            .classify(
                vec!["src/lib.rs".to_owned()],
                Some(repo.track_id.clone()),
                repo.items_dir.clone(),
            )
            .expect("a changed configured path must classify successfully");
        assert_eq!(classifications, vec![("src/lib.rs".to_owned(), "cli_composition".to_owned())]);

        let files = service
            .files("cli_composition".to_owned(), Some(repo.track_id.clone()), repo.items_dir)
            .expect("a configured scope must list its changed files");
        assert_eq!(files, vec!["src/lib.rs"]);
    }

    #[test]
    fn test_review_service_persist_commit_hash_rejects_invalid_track() {
        use usecase::review_v2::aggregate_service::ReviewService as _;

        let service = super::shim::review_service_impl();
        assert!(service.persist_commit_hash("../invalid".to_owned(), PathBuf::from(".")).is_err());
    }

    #[test]
    fn test_review_service_persist_commit_hash_records_current_head() {
        use usecase::review_v2::aggregate_service::ReviewService as _;

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-service-persist-happy-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();
        let service = super::shim::review_service_impl();

        let persisted = service
            .persist_commit_hash(repo.track_id.clone(), repo._dir.path().to_path_buf())
            .expect("the active track must persist its current head");
        let head = git_stdout(repo._dir.path(), &["rev-parse", "HEAD"]);

        assert_eq!(persisted, head);
        assert_eq!(
            fs::read_to_string(repo.track_dir.join(".commit_hash"))
                .expect("persisted commit hash must be readable")
                .trim(),
            head
        );
    }

    #[test]
    fn test_review_service_results_all_enumerates_universe_and_named_displays_only_selected_scope()
    {
        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-service-aux-results-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        fs::write(
            repo._dir.path().join(".harness/config/review-scope.json"),
            r#"{"version":2,"groups":{"cli_composition":{"patterns":["src/**"]},"infra":{"patterns":["infra/**"]}}}"#,
        )
        .unwrap();

        let driver = ReviewCompositionRoot::new().review_driver();
        let results_input = |scope: Option<&str>, all| {
            ReviewResultsInput::try_new(
                Some(repo.track_id.clone()),
                repo.items_dir.clone(),
                scope.map(str::to_owned),
                all,
                0,
                "fast".to_owned(),
                true,
            )
            .expect("fixture selector must be valid")
        };

        for (description, all) in [("omitted selector", false), ("explicit --all", true)] {
            let outcome = driver.handle(ReviewInput::Results(results_input(None, all)));
            assert_eq!(outcome.exit_code, 0, "{description} must succeed: {outcome:?}");
            let all_rendered = outcome.stdout.expect("successful results must render stdout");
            assert!(
                all_rendered.contains("Review results"),
                "summary header must be rendered, got: {all_rendered}"
            );
            for scope in ["cli_composition", "infra", "other"] {
                assert!(
                    all_rendered.contains(&format!(" {scope}:")),
                    "{description} must render configured scope {scope}; got: {all_rendered}"
                );
            }
            assert!(
                all_rendered.contains("3 total"),
                "{description} must use the complete configured scope universe; got: {all_rendered}"
            );
        }

        let outcome =
            driver.handle(ReviewInput::Results(results_input(Some("cli_composition"), false)));
        assert_eq!(outcome.exit_code, 0, "named selector must succeed: {outcome:?}");
        let named_rendered = outcome.stdout.expect("successful results must render stdout");
        assert!(
            named_rendered.contains(" cli_composition:"),
            "Named must render the selected scope; got: {named_rendered}"
        );
        for excluded_scope in ["infra", "other"] {
            assert!(
                !named_rendered.contains(&format!(" {excluded_scope}:")),
                "Named must not render unselected scope {excluded_scope}; got: {named_rendered}"
            );
        }
        assert!(
            named_rendered.contains("1 total"),
            "Named must render exactly one scope; got: {named_rendered}"
        );
    }

    #[test]
    fn test_review_results_service_returns_structured_named_and_all_universes() {
        use usecase::review_v2::{
            ReviewRoundResultVerdict, ReviewRoundType, ReviewScopeResultState,
            ReviewScopeSelectionRequest,
        };

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-results-service-structured-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        fs::write(
            repo._dir.path().join(".harness/config/review-scope.json"),
            r#"{"version":2,"groups":{"cli_composition":{"patterns":["src/**"]},"infra":{"patterns":["infra/**"]}}}"#,
        )
        .unwrap();
        fs::write(
            repo.track_dir.join("review.json"),
            r#"{
  "schema_version": 2,
  "scopes": {
    "cli_composition": {
      "rounds": [
        {
          "type": "fast",
          "verdict": "zero_findings",
          "findings": [],
          "hash": "rvw1:sha256:abcdef0123456789",
          "at": "2026-08-10T12:34:56Z"
        },
        {
          "type": "final",
          "verdict": "findings_remain",
          "findings": [{
            "message": "preserved finding",
            "severity": "medium",
            "file": "src/lib.rs",
            "line": 1,
            "category": "correctness"
          }],
          "hash": "rvw1:sha256:abcdef0123456789",
          "at": "2026-08-10T12:35:56Z"
        }
      ]
    },
    "infra": {
      "rounds": [{
        "type": "final",
        "verdict": "zero_findings",
        "findings": [],
        "hash": "rvw1:sha256:abcdef0123456789",
        "at": "2026-08-10T12:36:56Z"
      }]
    }
  }
}"#,
        )
        .unwrap();

        let service = super::shim::review_results_service();
        assert!(
            ReviewScopeSelectionRequest::try_new(Some("cli_composition".to_owned()), true).is_err(),
            "a named selector and --all must be rejected before results execution"
        );
        assert!(
            ReviewScopeSelectionRequest::try_new(Some(String::new()), false).is_err(),
            "an invalid scope spelling must be rejected before results execution"
        );
        let all_output = service
            .results(
                Some(repo.track_id.clone()),
                repo.items_dir.clone(),
                ReviewScopeSelectionRequest::All,
            )
            .expect("all selection must return structured results");
        let all_scopes = all_output
            .scopes
            .iter()
            .map(|output| output.scope.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(all_scopes, vec!["cli_composition", "infra", "other"]);
        let cli_scope = all_output
            .scopes
            .iter()
            .find(|output| output.scope.as_str() == "cli_composition")
            .expect("all selection must retain the configured cli_composition scope");
        assert!(matches!(cli_scope.state, ReviewScopeResultState::RequiredFindingsRemain));
        assert_eq!(cli_scope.rounds.len(), 2);
        assert!(matches!(
            cli_scope.rounds.first(),
            Some(round)
                if round.round_type == ReviewRoundType::Fast
                    && round.at == "2026-08-10T12:34:56Z"
                    && matches!(round.verdict, ReviewRoundResultVerdict::ZeroFindings)
        ));
        assert!(matches!(
            cli_scope.rounds.get(1),
            Some(round)
                if round.round_type == ReviewRoundType::Final
                    && round.at == "2026-08-10T12:35:56Z"
                    && matches!(
                        &round.verdict,
                        ReviewRoundResultVerdict::FindingsRemain(findings)
                            if findings.as_slice().len() == 1
                                && findings
                                    .as_slice()
                                    .first()
                                    .is_some_and(|finding| finding.message.as_str() == "preserved finding")
                    )
        ));
        let infra_scope = all_output
            .scopes
            .iter()
            .find(|output| output.scope.as_str() == "infra")
            .expect("all selection must retain the configured infra scope");
        assert!(matches!(infra_scope.state, ReviewScopeResultState::Empty));
        assert!(matches!(
            infra_scope.rounds.as_slice(),
            [round]
                if round.round_type == ReviewRoundType::Final
                    && round.at == "2026-08-10T12:36:56Z"
                    && matches!(round.verdict, ReviewRoundResultVerdict::ZeroFindings)
        ));
        let other_scope = all_output
            .scopes
            .iter()
            .find(|output| output.scope.as_str() == "other")
            .expect("all selection must retain the other scope");
        assert!(matches!(other_scope.state, ReviewScopeResultState::RequiredNotStarted));
        assert!(other_scope.rounds.is_empty());

        let named_output = service
            .results(
                Some(repo.track_id.clone()),
                repo.items_dir.clone(),
                ReviewScopeSelectionRequest::try_new(Some("cli_composition".to_owned()), false)
                    .expect("configured name must form a request"),
            )
            .expect("named selection must return structured results");
        assert_eq!(named_output.scopes.len(), 1);
        let named_scope =
            named_output.scopes.first().expect("named selection must contain its configured scope");
        assert_eq!(named_scope.scope.as_str(), "cli_composition");
        assert!(matches!(named_scope.state, ReviewScopeResultState::RequiredFindingsRemain));
        assert!(matches!(
            named_scope.rounds.first(),
            Some(round)
                if round.round_type == ReviewRoundType::Fast
                    && round.at == "2026-08-10T12:34:56Z"
                    && matches!(round.verdict, ReviewRoundResultVerdict::ZeroFindings)
        ));
        assert!(matches!(
            named_scope.rounds.get(1),
            Some(round)
                if round.round_type == ReviewRoundType::Final
                    && round.at == "2026-08-10T12:35:56Z"
                    && matches!(
                        &round.verdict,
                        ReviewRoundResultVerdict::FindingsRemain(findings)
                            if findings.as_slice().len() == 1
                                && findings
                                    .as_slice()
                                    .first()
                                    .is_some_and(|finding| finding.message.as_str() == "preserved finding")
                    )
        ));

        let unknown_scope =
            ReviewScopeSelectionRequest::try_new(Some("not-configured".to_owned()), false)
                .expect("format-valid unconfigured scope must form a named request");
        let error = service
            .results(Some(repo.track_id.clone()), repo.items_dir.clone(), unknown_scope)
            .expect_err("an unconfigured named scope must be rejected by selection resolution");
        assert!(matches!(
            error,
            usecase::review_v2::ReviewResultsError::UnknownScope(scope)
                if scope.as_str() == "not-configured"
        ));
    }

    #[test]
    fn test_review_results_driver_renders_real_adapter_fixture_round_history_and_findings() {
        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-results-rendered-fixture-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        fs::write(
            repo.track_dir.join("review.json"),
            r#"{
  "schema_version": 2,
  "scopes": {
    "cli_composition": {
      "rounds": [
        {
          "type": "fast",
          "verdict": "zero_findings",
          "findings": [],
          "hash": "rvw1:sha256:abcdef0123456789",
          "at": "2026-08-11T12:00:00Z"
        },
        {
          "type": "final",
          "verdict": "zero_findings",
          "findings": [],
          "hash": "rvw1:sha256:abcdef0123456789",
          "at": "2026-08-11T12:01:00Z"
        },
        {
          "type": "final",
          "verdict": "findings_remain",
          "findings": [{
            "message": "fixture finding detail",
            "severity": "P1",
            "file": "src/lib.rs",
            "line": 27,
            "category": "correctness"
          }],
          "hash": "rvw1:sha256:abcdef0123456789",
          "at": "2026-08-11T12:02:00Z"
        }
      ]
    }
  }
}"#,
        )
        .unwrap();

        let input = ReviewResultsInput::try_new(
            Some(repo.track_id.clone()),
            repo.items_dir.clone(),
            Some("cli_composition".to_owned()),
            false,
            3,
            "any".to_owned(),
            true,
        )
        .expect("fixture results input must be valid");
        let outcome =
            ReviewCompositionRoot::new().review_driver().handle(ReviewInput::Results(input));

        assert_eq!(outcome.exit_code, 0, "results fixture must render: {outcome:?}");
        let rendered = outcome.stdout.expect("results fixture must render stdout");
        assert!(rendered.contains("final@2026-08-11T12:02:00Z findings_remain"));
        assert!(rendered.contains("fixture finding detail (src/lib.rs:27)"));
        assert!(rendered.contains("history (newer first, up to --limit):"));
        assert!(rendered.contains("final@2026-08-11T12:01:00Z zero_findings"));
        assert!(rendered.contains("fast@2026-08-11T12:00:00Z zero_findings"));
    }

    #[test]
    fn test_review_results_service_exposes_named_and_all_results() {
        use usecase::review_v2::ReviewScopeSelectionRequest;

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-service-aggregate-results-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        fs::write(
            repo._dir.path().join(".harness/config/review-scope.json"),
            r#"{"version":2,"groups":{"cli_composition":{"patterns":["src/**"]},"infra":{"patterns":["infra/**"]}}}"#,
        )
        .unwrap();

        let service = super::shim::review_results_service();
        let all_output = service
            .results(
                Some(repo.track_id.clone()),
                repo.items_dir.clone(),
                ReviewScopeSelectionRequest::All,
            )
            .expect("results port must expose all-selection results");
        let all_scopes = all_output
            .scopes
            .iter()
            .map(|output| output.scope.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(all_scopes, vec!["cli_composition", "infra", "other"]);

        let named_output = service
            .results(
                Some(repo.track_id.clone()),
                repo.items_dir.clone(),
                ReviewScopeSelectionRequest::try_new(Some("cli_composition".to_owned()), false)
                    .expect("configured name must form a request"),
            )
            .expect("results port must expose named-selection results");
        assert_eq!(named_output.scopes.len(), 1);
        assert_eq!(
            named_output
                .scopes
                .first()
                .expect("aggregate named selection must contain its configured scope")
                .scope
                .as_str(),
            "cli_composition"
        );
    }

    #[test]
    fn test_review_service_results_unknown_named_scope_returns_membership_error() {
        use usecase::review_v2::ReviewScopeSelectionRequest;

        let _lock = cwd_lock().lock().unwrap();
        let repo = setup_review_entrypoint_repo("review-service-results-unknown-scope-2026");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo._dir.path()).unwrap();

        let request = ReviewScopeSelectionRequest::try_new(Some("no-such-scope".to_owned()), false)
            .expect("format-valid but unconfigured scope must form a named request");
        let error = super::shim::review_results_service()
            .results(Some(repo.track_id.clone()), repo.items_dir.clone(), request)
            .expect_err("an unconfigured named scope must be rejected");

        assert!(matches!(
            error,
            usecase::review_v2::ReviewResultsError::UnknownScope(scope)
                if scope.as_str() == "no-such-scope"
        ));
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
        let msg = match result {
            Err(error) => error.to_string(),
            Ok(_) => panic!("expected Err on non-track branch"),
        };
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

        let result = crate::review_v2::ReviewCompositionRoot::new().review_run_local_ungated(
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

        assert!(result.is_err(), "expected unsupported provider error");
        let msg = match result {
            Err(error) => error.to_string(),
            Ok(_) => panic!("expected unsupported provider error"),
        };
        assert!(
            msg.contains("unsupported reviewer provider 'gemini'"),
            "expected unsupported provider error, got: {msg}"
        );
    }

    /// Pin that the driver entry point is gate-aware: a
    /// failing configured pre-review command blocks the flow before any
    /// reviewer provider is consulted (an inner launch would fail with an
    /// unsupported-provider error instead of the pre-review block).
    #[cfg(unix)]
    #[test]
    fn review_run_local_public_entry_blocks_on_failing_pre_review_gate() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".harness/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("pre-review-gates.json"),
            r#"{
                "schema_version": 1,
                "scopes": [{
                    "scope": "cli_composition",
                    "commands": [{"argv": ["false", "pre-review-gate-evidence"], "timeout_seconds": null}]
                }]
            }"#,
        )
        .unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();
        GitRunner::at(dir.path()).assert_success(&["init", "-b", "main"]);
        GitRunner::at(dir.path()).assert_success(&["config", "user.email", "test@example.invalid"]);
        GitRunner::at(dir.path()).assert_success(&["config", "user.name", "test"]);
        GitRunner::at(dir.path()).assert_success(&["commit", "--allow-empty", "-qm", "initial"]);
        GitRunner::at(dir.path()).assert_success(&["checkout", "-qb", "track/gate-block-track"]);

        let outcome = crate::review_v2::ReviewCompositionRoot::new().review_driver().handle(
            cli_driver::review::ReviewInput::RunLocal(
                None,
                10,
                None,
                Some("Review.".to_owned()),
                Some("gate-block-track".to_owned()),
                "fast".to_owned(),
                "cli_composition".to_owned(),
                items_dir,
            ),
        );

        assert_eq!(outcome.exit_code, 1);
        let stderr = outcome.stderr.as_deref().unwrap_or("");
        assert!(
            stderr.contains("pre-review command failed"),
            "expected the pre-review gate block, got: {stderr}"
        );
        assert!(
            stderr.contains("pre-review-gate-evidence"),
            "expected the pre-review gate block to identify the failed argv, got: {stderr}"
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
        let msg = match result {
            Err(error) => error.to_string(),
            Ok(_) => panic!("expected Err on non-track branch"),
        };
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
        let msg = match result {
            Err(error) => error.to_string(),
            Ok(_) => panic!("expected Err on non-track branch"),
        };
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
        let msg = match result {
            Err(error) => error.to_string(),
            Ok(_) => panic!("expected Err on non-track branch"),
        };
        // The error must be a branch error ("not a track branch", "main", or similar)
        // rather than a git-discovery error ("failed to run git", "No such file", etc.).
        assert!(
            msg.contains("not a track branch") || msg.contains("main"),
            "expected branch error, got: {msg}"
        );
    }
}
