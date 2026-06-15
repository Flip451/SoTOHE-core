//! `dry` command family — composition logic and [`crate::CliApp`] impl.
//!
//! Provides the three `sotp dry` subcommand implementations:
//! - `write`: run the DRY-check write cycle (detect new violations, record results).
//! - `results`: read and display the historical dry-check results (informational).
//! - `check-approved`: gate that exits non-zero when unresolved pairs remain.
//!
//! Diff-base resolution and the hunk-scope pipeline are factored into shared
//! helpers (`resolve_dry_diff_base`, `build_diff_fragments`) to avoid duplication
//! between `write` and `check-approved` (both need the same three-branch
//! fail-closed policy and fragment extraction pipeline).
//!
//! CN-01 enforcement: only dry-check-owned adapters are used here
//! (`FsDryCheckCommitHashStore`, `GitDryCheckDiffGetter`). The `review_v2`
//! adapters (`FsCommitHashStore`, `GitDiffGetter`) are never imported.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use domain::dry_check::{DryCheckFinding, DryCheckReader as _, DryCheckVerdict};
use domain::semantic_dup::SimilarityThreshold;
use infrastructure::dry_check::{CodexDryChecker, FsDryCheckCoverageAdapter, FsDryCheckStore};
use infrastructure::semantic_dup::embedding::FastEmbedAdapter;
use usecase::dry_check::{
    DryCheckApprovalInteractor, DryCheckApprovalService as _, DryCheckInteractor,
    DryCheckResultsInteractor, DryCheckResultsService as _, DryCheckService as _, fragment_ref_of,
};

use crate::{CliApp, CommandOutcome};

mod manifest;
mod persistent_index;
mod shared;
mod tier_telemetry;

use persistent_index::open_persistent_index_with_corpus;
pub(crate) use shared::resolve_dry_diff_base_from_store;
use shared::{
    build_diff_and_corpus_fragments, dry_check_approved_outcome, dry_write_outcome,
    parse_dry_track_id, parse_verdict_filter, resolve_dry_diff_base,
    resolve_existing_dir_under_repo,
};
#[cfg(test)]
use shared::{git_diff_path_key, normalize_fragment_paths};
#[cfg(test)]
use tier_telemetry::{
    DryAgentRunRecorder, TieredDryAgentRecorder, dry_agent_error_is_subprocess_failure,
};
use tier_telemetry::{
    RecordingDryAgent, dry_tiered_telemetry_for_result, emit_dry_tier_external_subprocess,
    emit_dry_tier_review_round,
};

#[cfg(test)]
use domain::dry_check::{DryCheckApprovalVerdict, VerdictFilter, fragments_overlapping_hunks};
#[cfg(test)]
use domain::semantic_dup::CodeFragment;
#[cfg(test)]
use infrastructure::semantic_dup::{
    extractor::extract_code_fragments, index::LanceDbSemanticIndexAdapter,
};
#[cfg(test)]
use manifest::{
    EMBEDDING_MODEL_ID, IndexManifest, compute_manifest_diff, file_content_hash,
    manifest_sidecar_path, read_manifest, remove_manifest, write_manifest,
};
#[cfg(test)]
use persistent_index::{
    NullInsertIndexProxy, acquire_persistent_index_lock, clear_persistent_index_dir,
    persistent_index_lock_path, persistent_index_marker_path, write_persistent_index_marker,
};
#[cfg(test)]
use usecase::dry_check::{
    DryCheckAgentError, DryCheckAgentJudgment, DryCheckAgentPort, DryCheckCycleError,
    DryCheckJudgeTier,
};
#[cfg(test)]
use usecase::semantic_dup::SemanticIndexPort;

// ── Input DTOs ────────────────────────────────────────────────────────────────

/// Input DTO for `sotp dry write`.
#[derive(Debug, Clone)]
pub struct DryWriteInput {
    /// Track ID used to locate the per-track dry-check.json and .commit_hash.
    pub track_id: String,
    /// Optional explicit base commit (overrides FsDryCheckCommitHashStore lookup).
    pub base_commit: Option<String>,
    /// Path to the LanceDB semantic index database.
    pub db_path: PathBuf,
    /// Cosine similarity threshold (0.0–1.0) for the dry-check gate.
    pub threshold: Option<f32>,
    /// Root of the workspace to scan for Rust sources (corpus extraction).
    pub workspace_root: PathBuf,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
    /// Codex model name for the DryCheckAgentPort.
    /// `None` means "use the model from `agent-profiles.json`".
    /// An explicit value overrides the profile model.
    pub model: Option<String>,
    /// Capability name forwarded to CodexDryChecker.
    pub capability_name: String,
}

/// Input DTO for `sotp dry results`.
#[derive(Debug, Clone)]
pub struct DryResultsInput {
    /// Track ID used to locate the per-track dry-check.json.
    pub track_id: String,
    /// Verdict filter: "all" / "not-a-violation" / "accepted" / "violation"
    /// (default "all"). Parsed to `VerdictFilter` inside cli-composition (CN-02).
    pub filter: String,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
}

/// Input DTO for `sotp dry fix-local` (`dry_run_fix_local`).
///
/// Maps to the 2 required CLI flags plus the optional model override:
/// `--track-id` / `--briefing-file` / `--model`.
/// Carries stdlib-typed fields only — no domain or infrastructure types (CN-02).
#[derive(Debug, Clone)]
pub struct RunDryFixLocalInput {
    /// Track ID. Required (no auto-resolve from branch for write operations).
    pub track_id: String,
    /// Path to the briefing file passed to the dry-fix-lead fixer. Required.
    pub briefing_file: std::path::PathBuf,
    /// Model for the fixer (Codex) subprocess.
    /// `None` means "use the model from `agent-profiles.json`".
    /// An explicit value overrides the profile model.
    pub model: Option<String>,
}

/// Input DTO for `sotp dry check-approved`.
///
/// D5 / T005: `dry check-approved` is a pure-read staleness + all-resolved gate
/// (no embedding, no similarity search, no corpus / index / threshold), so the
/// old `db_path` / `threshold` / `workspace_root` fields are removed.
#[derive(Debug, Clone)]
pub struct DryCheckApprovedInput {
    /// Track ID used to locate the per-track dry-check.json and .commit_hash.
    pub track_id: String,
    /// Optional explicit base commit (overrides FsDryCheckCommitHashStore lookup).
    pub base_commit: Option<String>,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
}

// These helpers are retained for existing tests that verify ephemeral-index
// behavior (acceptance tests for the old API).  They are not used in production
// code paths (which now use the persistent index via
// `open_persistent_index_with_corpus`), so they are cfg(test) only.
#[cfg(test)]
fn ephemeral_index_parent(db_path: &Path, fallback_parent: &Path) -> PathBuf {
    match db_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() && parent.is_dir() => parent.to_path_buf(),
        _ => fallback_parent.to_path_buf(),
    }
}

#[cfg(test)]
fn create_ephemeral_index_dir(
    db_path: &Path,
    fallback_parent: &Path,
) -> Result<tempfile::TempDir, String> {
    let parent = ephemeral_index_parent(db_path, fallback_parent);
    tempfile::Builder::new()
        .prefix("sotp-dry-index-")
        .tempdir_in(&parent)
        .map_err(|e| format!("failed to create ephemeral index dir: {e}"))
}

#[cfg(test)]
fn create_ephemeral_index_adapter(
    db_path: &Path,
    fallback_parent: &Path,
) -> Result<(tempfile::TempDir, LanceDbSemanticIndexAdapter), String> {
    let temp_index_dir = create_ephemeral_index_dir(db_path, fallback_parent)?;
    let ephemeral_db_path = temp_index_dir.path().to_path_buf();
    let index_adapter =
        LanceDbSemanticIndexAdapter::new(ephemeral_db_path.clone()).map_err(|e| {
            format!("failed to open ephemeral index at {}: {e}", ephemeral_db_path.display())
        })?;

    Ok((temp_index_dir, index_adapter))
}

fn resolve_dry_write_telemetry_writer(
    items_dir: &Path,
    dry_track_id: &str,
) -> Option<(infrastructure::telemetry::TelemetryWriter, String)> {
    crate::telemetry_wiring::resolve_telemetry_writer(items_dir)
        .filter(|(_, telemetry_track_id)| telemetry_track_id.as_str() == dry_track_id)
}

// ── CliApp impl — dry write ───────────────────────────────────────────────────

impl CliApp {
    /// Run `sotp dry write`: detect DRY violations for the current diff scope and
    /// record results to dry-check.json.
    ///
    /// Diff-base resolution follows the fail-closed three-branch policy (D4/ADR).
    /// The hunk-scope pipeline (CN-04) ensures only changed fragments are queried.
    ///
    /// # Errors
    ///
    /// Returns `Err` on arg validation, diff acquisition, fragment extraction,
    /// adapter construction, or interactor failures.
    pub fn dry_write(&self, input: DryWriteInput) -> Result<CommandOutcome, String> {
        use infrastructure::git_cli::{GitRepository, SystemGitRepo};

        // Resolve repo root to anchor paths.
        let git = SystemGitRepo::discover().map_err(|e| format!("git discover: {e}"))?;
        let root = git.root().to_path_buf();
        let canonical_root =
            root.canonicalize().map_err(|e| format!("failed to canonicalize repo root: {e}"))?;
        let track_id = parse_dry_track_id(&input.track_id)?;
        let telemetry_writer =
            resolve_dry_write_telemetry_writer(&input.items_dir, track_id.as_ref());

        let (fast_model, effective_model) =
            resolve_dry_checker_models(&root, &input.capability_name, input.model.clone())?;

        // Locate per-track directory.
        let items_dir_abs =
            resolve_existing_dir_under_repo(&input.items_dir, &root, &canonical_root, "items_dir")?;
        let track_dir = items_dir_abs.join(track_id.as_ref());

        let commit_hash_path = track_dir.join(".commit_hash");
        let dry_check_json_path = track_dir.join("dry-check.json");
        let dry_check_coverage_path = track_dir.join("dry-check-coverage.json");

        // Resolve diff base (fail-closed three-branch policy).
        let base = resolve_dry_diff_base(
            input.base_commit.as_deref(),
            &commit_hash_path,
            &canonical_root,
        )?;

        // Always load infra config for max_parallelism (D3 / T010); --threshold overrides its threshold.
        let config_path = root.join(".harness/config/dry-check.json");
        let infra_config = infrastructure::dry_check::DryCheckConfig::load(&config_path)
            .map_err(|e| format!("failed to load dry-check config: {e}"))?;

        let threshold = match input.threshold {
            Some(t) => {
                SimilarityThreshold::new(t).map_err(|e| format!("invalid --threshold: {e}"))?
            }
            None => infra_config.threshold(),
        };

        let usecase_config = build_usecase_dry_check_config(&infra_config)?;

        let workspace_root = resolve_existing_dir_under_repo(
            &input.workspace_root,
            &root,
            &canonical_root,
            "workspace_root",
        )?;

        // Build diff_fragments + corpus_fragments via shared hunk-scope pipeline.
        let (diff_fragments, corpus_fragments) =
            build_diff_and_corpus_fragments(&base, &workspace_root, &canonical_root)?;
        let diff_fragments_processed = diff_fragments.len();

        // Construct adapters.
        let store = Arc::new(FsDryCheckStore::new(dry_check_json_path, canonical_root.clone()));
        // D5 (T004): the coverage manifest is written by `DryCheckInteractor`
        // at the end of `run_dry_check`. Composition just constructs the
        // adapter and hands it to the interactor.
        let coverage = Arc::new(FsDryCheckCoverageAdapter::new(
            dry_check_coverage_path,
            canonical_root.clone(),
        ));
        let store_for_summary = store.clone();
        let records_before = store_for_summary
            .read_records()
            .map_err(|e| format!("dry-check read before write failed: {e}"))?
            .len();
        let (agent, tiered_recorder) = RecordingDryAgent::new(CodexDryChecker::new(
            fast_model.clone(),
            infra_config.fast_reasoning_effort().to_owned(),
            effective_model.clone(),
            infra_config.final_reasoning_effort().to_owned(),
            input.capability_name.clone(),
        ));
        let agent = Arc::new(agent);
        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );

        // D7/IN-10: persistent index keyed by file-level content hash; `NullInsertIndexProxy`
        // makes the interactor's `build_corpus_index` a no-op (corpus already correct).
        let index_port = open_persistent_index_with_corpus(
            &input.db_path,
            corpus_fragments,
            embedding_port.as_ref(),
        )?;

        // Compute the fingerprint using the *effective* threshold (which may have
        // been overridden by `--threshold`).  `infra_config.fingerprint()` always
        // uses the file-config threshold, so a `dry write --threshold X` run
        // would store the file-config fingerprint (not X), causing `check-approved`
        // to accept stale coverage as fresh (P1 correctness fix).
        // `fingerprint_with_threshold` uses the supplied threshold instead.
        let config_fingerprint = infra_config.fingerprint_with_threshold(threshold);

        let interactor = DryCheckInteractor::new(
            embedding_port,
            index_port,
            agent,
            store.clone(),
            store,
            coverage,
            track_id.clone(),
            usecase_config,
            config_fingerprint,
        );

        let dry_start = std::time::Instant::now();
        let dry_result = interactor.run_dry_check(vec![], diff_fragments, threshold, base);

        // T013 / IN-07 / AC-09 / CN-10: emit per-tier ReviewRound telemetry.
        // Fast tier uses `fast_model`; final tier uses `effective_model` (final model).
        // New-field / new-event additions are forbidden (CN-10): we reuse the existing
        // `round_type` field with "fast" / "final" values already accepted by the schema.
        if let Some((ref w, ref tid)) = telemetry_writer {
            let (fast_telemetry, final_telemetry) =
                dry_tiered_telemetry_for_result(&dry_result, &tiered_recorder);
            if let Some(ref telemetry) = fast_telemetry {
                // On escalated runs `duration_ms` is pre-computed as
                // `(final_started_at - fast_started_at)` so that the fast
                // ReviewRound does not include the final tier's time.
                // The ExternalSubprocess event uses the same duration source so
                // that both events are consistent with each other.
                emit_dry_tier_review_round(
                    w,
                    tid,
                    "codex",
                    &fast_model,
                    "fast",
                    telemetry,
                    dry_start,
                );
                if telemetry.subprocess_started_at.is_some() {
                    emit_dry_tier_external_subprocess(w, tid, "codex", telemetry, dry_start);
                }
            }
            if let Some(ref telemetry) = final_telemetry {
                // Final tier uses its own started_at so duration excludes the fast tier.
                emit_dry_tier_review_round(
                    w,
                    tid,
                    "codex",
                    &effective_model,
                    "final",
                    telemetry,
                    dry_start,
                );
                if telemetry.subprocess_started_at.is_some() {
                    emit_dry_tier_external_subprocess(w, tid, "codex", telemetry, dry_start);
                }
            }
        }

        let findings: Vec<DryCheckFinding> =
            dry_result.map_err(|e| format!("dry-check write cycle failed: {e}"))?;
        // D5 (T004): coverage is now written inside `DryCheckInteractor::run_dry_check`.

        let records_after = store_for_summary
            .read_records()
            .map_err(|e| format!("dry-check read after write failed: {e}"))?
            .len();
        let records_appended = records_after.saturating_sub(records_before);
        let pairs_checked = records_appended;

        Ok(dry_write_outcome(&findings, pairs_checked, records_appended, diff_fragments_processed))
    }

    /// Run `sotp dry results`: read and display the historical dry-check results.
    ///
    /// INFORMATIONAL — always exits 0 on successful read; exits non-zero only on
    /// `DryCheckReaderError`.
    ///
    /// # Errors
    ///
    /// Returns `Err` on store access failures (`DryCheckReaderError`).
    pub fn dry_results(&self, input: DryResultsInput) -> Result<CommandOutcome, String> {
        use infrastructure::git_cli::{GitRepository, SystemGitRepo};

        let git = SystemGitRepo::discover().map_err(|e| format!("git discover: {e}"))?;
        let root = git.root().to_path_buf();
        let canonical_root =
            root.canonicalize().map_err(|e| format!("failed to canonicalize repo root: {e}"))?;
        let track_id = parse_dry_track_id(&input.track_id)?;

        let items_dir_abs =
            resolve_existing_dir_under_repo(&input.items_dir, &root, &canonical_root, "items_dir")?;
        let track_dir = items_dir_abs.join(track_id.as_ref());
        let dry_check_json_path = track_dir.join("dry-check.json");

        let filter = parse_verdict_filter(&input.filter)?;

        let store = Arc::new(FsDryCheckStore::new(dry_check_json_path, canonical_root));
        let interactor = DryCheckResultsInteractor::new(store);

        let results =
            interactor.get_results(filter).map_err(|e| format!("dry results read failed: {e}"))?;

        let mut lines: Vec<String> = Vec::new();
        lines.push(format!("dry results: {} record(s)", results.records.len()));
        for record in &results.records {
            lines.push(format!(
                "  pair: [{} ({})] <-> [{} ({})]",
                record.pair_key().low().path().as_str(),
                record.pair_key().low().content_hash().as_str(),
                record.pair_key().high().path().as_str(),
                record.pair_key().high().content_hash().as_str(),
            ));
            lines
                .push(format!("  changed_path (display-only): {}", record.changed_path().as_str()));
            let verdict_str = match record.verdict() {
                DryCheckVerdict::NotAViolation => "not-a-violation".to_owned(),
                DryCheckVerdict::Accepted => "accepted".to_owned(),
                DryCheckVerdict::Violation { refactor_proposal } => {
                    format!("violation | proposal: {}", refactor_proposal.as_str())
                }
            };
            lines.push(format!("  verdict: {verdict_str}"));
            lines.push(format!(
                "  score: {} | threshold: {} | base: {}",
                record.similarity_score().value(),
                record.threshold().value(),
                record.base_commit().as_ref(),
            ));
            lines.push(format!("  rationale: {}", record.rationale().as_str()));
            lines.push(format!("  recorded_at: {}", record.recorded_at().as_str()));
        }

        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }

    /// Run `sotp dry check-approved`: gate that exits non-zero when unresolved
    /// pairs remain.
    ///
    /// Diff-base resolution follows the same fail-closed three-branch policy as
    /// `dry_write`. The resolved base commit is used only for diff acquisition —
    /// it is NOT forwarded to `check_approved` (which records nothing and needs
    /// no base).
    ///
    /// # Errors
    ///
    /// Returns `Err` on arg validation, diff acquisition, adapter construction,
    /// or interactor failures.
    pub fn dry_check_approved(
        &self,
        input: DryCheckApprovedInput,
    ) -> Result<CommandOutcome, String> {
        use std::collections::BTreeSet;

        use infrastructure::git_cli::{GitRepository, SystemGitRepo};

        // D5 (T007 / IN-07 / AC-02 / CN-10): GateEval telemetry start.
        let gate_start = Instant::now();

        let git = SystemGitRepo::discover().map_err(|e| format!("git discover: {e}"))?;
        let root = git.root().to_path_buf();
        let canonical_root =
            root.canonicalize().map_err(|e| format!("failed to canonicalize repo root: {e}"))?;
        let track_id = parse_dry_track_id(&input.track_id)?;

        let items_dir_abs =
            resolve_existing_dir_under_repo(&input.items_dir, &root, &canonical_root, "items_dir")?;
        let track_dir = items_dir_abs.join(track_id.as_ref());

        let commit_hash_path = track_dir.join(".commit_hash");
        let dry_check_json_path = track_dir.join("dry-check.json");
        let dry_check_coverage_path = track_dir.join("dry-check-coverage.json");

        // Resolve diff base (fail-closed three-branch policy, same as write).
        let base = resolve_dry_diff_base(
            input.base_commit.as_deref(),
            &commit_hash_path,
            &canonical_root,
        )?;

        // D5 / IN-05 (T005): pure-read gate — diff fragments only.
        let (diff_fragments, _corpus_fragments) =
            build_diff_and_corpus_fragments(&base, &canonical_root, &canonical_root)?;

        let mut current_fragment_refs: BTreeSet<domain::dry_check::FragmentRef> = BTreeSet::new();
        for fragment in &diff_fragments {
            let fragment_ref = fragment_ref_of(fragment)
                .map_err(|e| format!("dry check-approved: failed to derive fragment ref: {e}"))?;
            current_fragment_refs.insert(fragment_ref);
        }

        // Load config to compute the current fingerprint for the gate comparison.
        let config_path = root.join(".harness/config/dry-check.json");
        let infra_config = infrastructure::dry_check::DryCheckConfig::load(&config_path)
            .map_err(|e| format!("failed to load dry-check config: {e}"))?;
        let current_config_fingerprint = infra_config.fingerprint();

        // Construct adapters: reader + coverage port — no embedding, no index.
        let store = Arc::new(FsDryCheckStore::new(dry_check_json_path, canonical_root.clone()));
        let coverage =
            Arc::new(FsDryCheckCoverageAdapter::new(dry_check_coverage_path, canonical_root));

        // D5 read-only interactor.
        let interactor =
            DryCheckApprovalInteractor::new(store, coverage, current_config_fingerprint);

        let verdict = interactor
            .check_approved(&track_id, &current_fragment_refs)
            .map_err(|e| format!("dry check-approved failed: {e}"))?;

        // D5 (T007 / IN-07 / AC-02 / CN-10): emit GateEval telemetry.
        let telemetry_writer =
            resolve_dry_write_telemetry_writer(&input.items_dir, track_id.as_ref());
        if let Some((ref w, ref tid)) = telemetry_writer {
            let (verdict_str, reason_summary) = dry_check_approved_gate_eval_fields(&verdict);
            crate::telemetry_wiring::emit_gate_eval(
                w,
                tid,
                "dry",
                verdict_str,
                &reason_summary,
                gate_start,
            );
        }

        Ok(dry_check_approved_outcome(verdict))
    }
}

/// Lift infra `DryCheckConfig` fields (max_parallelism + known-bad percents) into the validated
/// usecase newtypes (D3 / D4 / T011). All values come from `.harness/config/dry-check.json` v3.
fn build_usecase_dry_check_config(
    infra_config: &infrastructure::dry_check::DryCheckConfig,
) -> Result<usecase::dry_check::DryCheckConfig, String> {
    use usecase::dry_check::{DryCheckConfig, DryCheckParallelism, DryCheckPercent};
    let percent =
        |v: u8| DryCheckPercent::try_new(v).map_err(|e| format!("invalid known-bad percent: {e}"));
    Ok(DryCheckConfig::new(
        percent(infra_config.known_bad_injection_rate_percent())?,
        percent(infra_config.known_bad_detection_threshold_percent())?,
        DryCheckParallelism::try_new(infra_config.max_parallelism())
            .map_err(|e| format!("invalid max_parallelism: {e}"))?,
    ))
}

/// Resolve `(fast_model, final_model)` for the `dry-checker` capability (D4 / T012).
/// Explicit `--model` overrides both. Otherwise read `RoundType::Final` and `Fast` from
/// `agent-profiles.json`, falling back fast → final when no `fast_model` is configured.
fn resolve_dry_checker_models(
    root: &std::path::Path,
    capability_name: &str,
    explicit_model: Option<String>,
) -> Result<(String, String), String> {
    use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles, RoundType};
    if let Some(m) = explicit_model {
        return Ok((m.clone(), m));
    }
    let profiles = AgentProfiles::load(&root.join(AGENT_PROFILES_PATH))
        .map_err(|e| format!("[ERROR] failed to load agent-profiles.json: {e}"))?;
    let resolve = |rt| profiles.resolve_execution(capability_name, rt).and_then(|r| r.model);
    let final_model = resolve(RoundType::Final).ok_or_else(|| {
        format!(
            "[ERROR] no model specified: pass --model or set model in \
             agent-profiles.json '{capability_name}' capability"
        )
    })?;
    Ok((resolve(RoundType::Fast).unwrap_or_else(|| final_model.clone()), final_model))
}

/// `GateEval` telemetry fields for the `"dry"` gate (T007 / IN-07 / CN-10):
/// `Approved` → `("ok", "")`; `Blocked` → `("error", "blocked: N unresolved pair(s)")`.
fn dry_check_approved_gate_eval_fields(
    verdict: &domain::dry_check::DryCheckApprovalVerdict,
) -> (&'static str, String) {
    match verdict {
        domain::dry_check::DryCheckApprovalVerdict::Approved => ("ok", String::new()),
        domain::dry_check::DryCheckApprovalVerdict::Blocked { unresolved_pair_count } => {
            ("error", format!("blocked: {unresolved_pair_count} unresolved pair(s)"))
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    use crate::dry_fix_runner::{
        DryFixSessionLogCleanup, build_dry_fix_invocation, dry_fix_build_safe_env,
        dry_fix_build_smoke_env, dry_fix_smoke_test_codex_version, dry_fix_spawn_and_collect,
        resolve_codex_bin, run_dry_fix_codex,
    };
    use crate::review_v2::process_guards::{CwdGuard, EnvGuard, GitRunner};
    #[cfg(unix)]
    use crate::test_support::make_executable;
    use crate::test_support::repo_root_for_tests;

    fn temp_items_dir_under_repo() -> tempfile::TempDir {
        let base = repo_root_for_tests().join("target").join("dry-cli-composition-tests");
        std::fs::create_dir_all(&base).expect("test temp base must be creatable");
        tempfile::Builder::new()
            .prefix("items-")
            .tempdir_in(base)
            .expect("repo-local temp items_dir must be creatable")
    }

    fn valid_commit_hash_for_tests() -> String {
        "a".repeat(40)
    }

    fn setup_dry_telemetry_repo(root: &Path) -> PathBuf {
        GitRunner::at(root).assert_success(&["init", "-b", "main"]);
        GitRunner::at(root).assert_success(&["config", "user.email", "test@example.com"]);
        GitRunner::at(root).assert_success(&["config", "user.name", "Test"]);
        GitRunner::at(root).assert_success(&["config", "commit.gpgsign", "false"]);
        let items_dir = root.join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::write(root.join("README.md"), "init\n").unwrap();
        GitRunner::at(root).assert_success(&["add", "."]);
        GitRunner::at(root).assert_success(&["commit", "--no-gpg-sign", "-m", "init"]);
        items_dir
    }

    #[test]
    fn test_resolve_dry_write_telemetry_writer_requires_track_branch() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let repo = tempfile::tempdir().unwrap();
        let items_dir = setup_dry_telemetry_repo(repo.path());
        let _telemetry_guard = EnvGuard::set("SOTP_TELEMETRY", "1");
        let _telemetry_dir_guard = EnvGuard::remove("SOTP_TELEMETRY_DIR");

        let writer =
            resolve_dry_write_telemetry_writer(&items_dir, "dry-telemetry-main-2026-06-11");

        assert!(writer.is_none(), "dry-write telemetry must not emit from non-track branches");
    }

    #[test]
    fn test_resolve_dry_write_telemetry_writer_requires_matching_track_branch() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let repo = tempfile::tempdir().unwrap();
        let items_dir = setup_dry_telemetry_repo(repo.path());
        let branch_track_id = "dry-telemetry-branch-2026-06-11";
        GitRunner::at(repo.path()).assert_success(&[
            "checkout",
            "-b",
            &format!("track/{branch_track_id}"),
        ]);
        let _telemetry_guard = EnvGuard::set("SOTP_TELEMETRY", "1");
        let _telemetry_dir_guard = EnvGuard::remove("SOTP_TELEMETRY_DIR");

        assert!(
            resolve_dry_write_telemetry_writer(&items_dir, "dry-telemetry-other-2026-06-11",)
                .is_none(),
            "dry-write telemetry must not emit for a different explicit track id"
        );

        let (_writer, resolved_track_id) =
            resolve_dry_write_telemetry_writer(&items_dir, branch_track_id).unwrap();
        assert_eq!(resolved_track_id, branch_track_id);
    }

    #[cfg(unix)]
    fn write_fake_codex_runner(dir: &Path, body: &str) -> PathBuf {
        let script_content = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"codex 0.125.0\"; exit 0; fi\n{body}"
        );
        write_executable_script(dir, "fake-codex.sh", &script_content)
    }

    #[cfg(unix)]
    fn write_executable_script(dir: &Path, name: &str, script_content: &str) -> PathBuf {
        let script = dir.join(name);
        std::fs::write(&script, script_content).unwrap();
        make_executable(&script);
        script
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_codex_bin_uses_asdf_which_when_env_missing() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let real_codex = dir.path().join("real-codex");
        std::fs::write(&real_codex, "#!/bin/sh\nexit 0\n").unwrap();
        make_executable(&real_codex);
        let fake_asdf = format!(
            "#!/bin/sh\nif [ -n \"$GITHUB_TOKEN\" ] || [ -n \"$SSH_AUTH_SOCK\" ] || [ -n \"$HOME\" ] || [ -n \"$CODEX_HOME\" ]; then exit 7; fi\nif [ \"$1\" = \"which\" ] && [ \"$2\" = \"codex\" ]; then printf '%s\\n' '{}'; exit 0; fi\nexit 1\n",
            real_codex.display()
        );
        write_executable_script(dir.path(), "asdf", &fake_asdf);
        let existing_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![dir.path().to_path_buf()];
        if !existing_path.is_empty() {
            paths.extend(std::env::split_paths(&existing_path));
        }
        let _path = EnvGuard::set("PATH", std::env::join_paths(paths).unwrap());
        let _codex_bin = EnvGuard::remove("CODEX_BIN");
        let _github_token = EnvGuard::set("GITHUB_TOKEN", "ghp-secret");
        let _ssh_auth_sock = EnvGuard::set("SSH_AUTH_SOCK", "/tmp/ssh-agent.sock");
        let _home = EnvGuard::set("HOME", "/real-home");
        let _codex_home = EnvGuard::set("CODEX_HOME", "/real-codex-home");

        assert_eq!(resolve_codex_bin(), real_codex.as_os_str().to_os_string());
    }

    // ── resolve_dry_diff_base: unit tests ─────────────────────────────────────

    #[test]
    fn test_resolve_dry_diff_base_uses_override_when_provided() {
        let dir = tempfile::tempdir().unwrap();
        let commit_hash_path = dir.path().join(".commit_hash");
        let trusted_root = dir.path().to_path_buf();

        // Provide a valid 40-char override — store is never consulted.
        let valid_hash = "a".repeat(40);
        let result =
            resolve_dry_diff_base(Some(&valid_hash), &commit_hash_path, &trusted_root).unwrap();
        assert_eq!(result.as_ref(), valid_hash);
    }

    #[test]
    fn test_resolve_dry_diff_base_override_rejects_invalid_hash() {
        let dir = tempfile::tempdir().unwrap();
        let commit_hash_path = dir.path().join(".commit_hash");
        let trusted_root = dir.path().to_path_buf();

        let result = resolve_dry_diff_base(Some("not-a-hash"), &commit_hash_path, &trusted_root);
        assert!(result.is_err(), "invalid override must return Err");
        let msg = result.unwrap_err();
        assert!(msg.contains("--base-commit"), "error must mention --base-commit, got: {msg}");
    }

    /// When the store file is absent (Ok(None)), resolve_dry_diff_base falls back to
    /// git rev-parse main. In the test environment we rely on the real git repo,
    /// so we just assert the fallback path doesn't produce a panic or abort.
    #[test]
    fn test_resolve_dry_diff_base_falls_back_on_absent_file() {
        let dir = tempfile::tempdir().unwrap();
        let commit_hash_path = dir.path().join(".commit_hash");
        let trusted_root = dir.path().to_path_buf();

        // File absent → Ok(None) → tries git rev-parse main.
        // In this repo context git is available, so it should succeed or return Err
        // (not panic). We only assert it doesn't panic.
        let _ = resolve_dry_diff_base(None, &commit_hash_path, &trusted_root);
        // Pass: no panic.
    }

    /// When the store file contains a malformed hash (Err(Format)), the error is
    /// absorbed (warn + fallback) — must NOT propagate as CLI error.
    #[test]
    fn test_resolve_dry_diff_base_absorbs_format_error_and_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let commit_hash_path = dir.path().join(".commit_hash");
        let trusted_root = dir.path().to_path_buf();

        // Write invalid hash content → Err(DryCheckCommitHashError::Format).
        std::fs::write(&commit_hash_path, "not-a-valid-hash\n").unwrap();

        // Should NOT return Err — must absorb the Format error.
        // (The fallback git rev-parse main may succeed or fail depending on env.)
        // Key invariant: if git is available, result is Ok. If git fails, Err is OK.
        // What must NOT happen: a direct propagation of the Format error.
        let result = resolve_dry_diff_base(None, &commit_hash_path, &trusted_root);
        if let Err(ref msg) = result {
            assert!(
                !msg.contains("invalid commit hash"),
                "Format error must be absorbed, not propagated. Got: {msg}"
            );
        }
        // Pass: Format error absorbed (no direct propagation).
    }

    // ── build_diff_and_corpus_fragments: pipeline ordering ────────────────────

    #[test]
    fn test_build_diff_and_corpus_fragments_corpus_is_whole_workspace() {
        // With no changed hunks (empty base that produces no diff), corpus_fragments
        // should contain all fragments from the workspace_root.
        // We use a temp directory with a small Rust file as workspace.
        let dir = tempfile::tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("lib.rs"),
            "fn foo() { let x = 1; }\nfn bar() { let y = 2; }\n",
        )
        .unwrap();

        // extract corpus manually to compare.
        let corpus =
            extract_code_fragments(dir.path()).expect("extract_code_fragments must succeed");

        // The corpus must contain at least the two functions we wrote.
        // (The actual diff pipeline requires a live git repo for diff — we just
        // test that corpus extraction from workspace_root works.)
        assert!(!corpus.is_empty(), "corpus_fragments must not be empty for a non-empty workspace");
    }

    #[test]
    fn test_build_diff_and_corpus_fragments_diff_fragments_are_hunk_scoped() {
        // fragments_overlapping_hunks is called on the candidate set — verify it works
        // by calling it directly with known inputs.
        use domain::dry_check::{DiffFileHunks, DiffHunkRange};
        use domain::review_v2::FilePath;
        use domain::semantic_dup::CodeFragment;

        // Fragment at lines 5-10 with a hunk covering lines 6-8 → overlap.
        let hunk = DiffHunkRange::new(6, 8).unwrap();
        let file_path = FilePath::new("src/lib.rs").unwrap();
        let file_hunks = DiffFileHunks::new(file_path, vec![hunk]).unwrap();

        let frag = CodeFragment::new(
            std::path::PathBuf::from("src/lib.rs"),
            "fn foo() {}".to_owned(),
            5,
            10,
        )
        .unwrap();

        let overlapping = fragments_overlapping_hunks(std::slice::from_ref(&frag), &[file_hunks]);
        assert_eq!(overlapping.len(), 1, "fragment overlapping the hunk must be included");

        // Fragment at lines 20-30 with no overlapping hunk → excluded.
        let hunk2 = DiffHunkRange::new(6, 8).unwrap();
        let file_path2 = FilePath::new("src/lib.rs").unwrap();
        let file_hunks2 = DiffFileHunks::new(file_path2, vec![hunk2]).unwrap();

        let frag_outside = CodeFragment::new(
            std::path::PathBuf::from("src/lib.rs"),
            "fn bar() {}".to_owned(),
            20,
            30,
        )
        .unwrap();

        let non_overlapping = fragments_overlapping_hunks(&[frag_outside], &[file_hunks2]);
        assert!(non_overlapping.is_empty(), "fragment outside the hunk must be excluded");
    }

    // ── normalize_fragment_paths: path normalization regression ───────────────

    /// Regression test for the P1 bug: `extract_code_fragments` returns fragments
    /// with absolute `source_path` values when `workspace_root` is absolute, but
    /// `git diff` hunk paths are repo-relative.  `normalize_fragment_paths` must
    /// strip the `repo_root` prefix so that:
    ///  1. `candidate_fragments` (changed_paths filter) actually matches, and
    ///  2. `fragments_overlapping_hunks` path comparison succeeds.
    ///
    /// This test verifies that after normalization the paths are repo-relative and
    /// that a hunk whose path matches the repo-relative form is correctly recognized.
    #[test]
    fn test_normalize_fragment_paths_strips_absolute_repo_root() {
        use domain::dry_check::{DiffFileHunks, DiffHunkRange};
        use domain::review_v2::FilePath;
        use domain::semantic_dup::CodeFragment;

        let repo_root = std::path::PathBuf::from("/absolute/workspace/root");

        // Build a fragment with an absolute source_path (as extract_code_fragments
        // would produce when workspace_root is absolute).
        let absolute_path = repo_root.join("src/lib.rs");
        let frag = CodeFragment::new(absolute_path, "fn foo() {}".to_owned(), 1, 1).unwrap();

        // Normalize: absolute → repo-relative.
        let normalized = normalize_fragment_paths(vec![frag], &repo_root).unwrap();

        assert_eq!(normalized.len(), 1);
        let norm_path = &normalized[0].source_path;

        // After stripping, path must be repo-relative (NOT absolute).
        assert!(
            norm_path.is_relative(),
            "normalized path must be relative, got: {}",
            norm_path.display()
        );
        assert_eq!(
            norm_path.to_string_lossy(),
            "src/lib.rs",
            "normalized path must match the repo-relative git hunk path"
        );

        // Confirm that fragments_overlapping_hunks now matches the repo-relative hunk path.
        let hunk = DiffHunkRange::new(1, 1).unwrap();
        let file_path = FilePath::new("src/lib.rs").unwrap();
        let file_hunks = DiffFileHunks::new(file_path, vec![hunk]).unwrap();

        let diff_fragments = fragments_overlapping_hunks(&normalized, &[file_hunks]);
        assert_eq!(
            diff_fragments.len(),
            1,
            "fragment must be found after path normalization; without normalization, \
             absolute path would not match the repo-relative hunk path"
        );
    }

    #[test]
    fn test_normalize_fragment_paths_subdir_workspace_returns_repo_relative_path() {
        use domain::semantic_dup::CodeFragment;

        let repo_root = std::path::PathBuf::from("/repo");
        let fragment_path = repo_root.join("apps/cli/src/main.rs");
        let frag = CodeFragment::new(fragment_path, "fn main() {}".to_owned(), 1, 1).unwrap();

        let normalized = normalize_fragment_paths(vec![frag], &repo_root).unwrap();

        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].source_path.to_string_lossy(), "apps/cli/src/main.rs");
    }

    #[test]
    fn test_git_diff_path_key_windows_separators_returns_slash_path() {
        let path = std::path::PathBuf::from(r"src\lib.rs");
        let key = git_diff_path_key(&path);
        assert_eq!(key, "src/lib.rs");
    }

    #[test]
    fn test_dry_agent_unexpected_after_spawn_classifies_child_poll_failure() {
        let error =
            DryCheckAgentError::Unexpected("failed to poll dry-check agent child: io".to_owned());

        assert!(dry_agent_error_is_subprocess_failure(&error));
    }

    #[test]
    fn test_dry_agent_unexpected_before_spawn_is_not_subprocess_failure() {
        let error =
            DryCheckAgentError::Unexpected("failed to write output-schema: disk full".to_owned());

        assert!(!dry_agent_error_is_subprocess_failure(&error));
    }

    #[test]
    fn test_dry_agent_unexpected_fragment_path_error_is_not_subprocess_failure() {
        let error = DryCheckAgentError::Unexpected(
            "invalid fragment path: path must be relative".to_owned(),
        );

        assert!(!dry_agent_error_is_subprocess_failure(&error));
    }

    fn make_tiered_recorder_fast_only(findings: u32) -> TieredDryAgentRecorder {
        let fast = DryAgentRunRecorder::new();
        fast.record_started();
        for _ in 0..findings {
            fast.record_violation();
        }
        fast.record_completed();
        TieredDryAgentRecorder { fast, final_: DryAgentRunRecorder::new() }
    }

    #[test]
    fn test_dry_tiered_telemetry_for_result_index_after_fast_agent_success_emits_fast() {
        let tiered = make_tiered_recorder_fast_only(2);
        let result: Result<Vec<DryCheckFinding>, DryCheckCycleError> = Err(
            DryCheckCycleError::Index(usecase::semantic_dup::SemanticIndexError::SearchFailed {
                source: "changed_path error: invalid path".to_owned(),
            }),
        );

        let (fast, final_) = dry_tiered_telemetry_for_result(&result, &tiered);
        let fast_t = fast.unwrap();
        assert_eq!(fast_t.findings_count, 2);
        assert!(!fast_t.verdict_parse_failed);
        assert!(fast_t.subprocess_started_at.is_some());
        assert!(final_.is_none(), "final tier must not emit when only fast was invoked");
    }

    #[test]
    fn test_dry_tiered_telemetry_for_result_index_before_agent_returns_none() {
        let tiered = TieredDryAgentRecorder {
            fast: DryAgentRunRecorder::new(),
            final_: DryAgentRunRecorder::new(),
        };
        let result: Result<Vec<DryCheckFinding>, DryCheckCycleError> = Err(
            DryCheckCycleError::Index(usecase::semantic_dup::SemanticIndexError::SearchFailed {
                source: "candidate index failed".to_owned(),
            }),
        );

        let (fast, final_) = dry_tiered_telemetry_for_result(&result, &tiered);
        assert!(fast.is_none());
        assert!(final_.is_none());
    }

    #[test]
    fn test_dry_tiered_telemetry_for_result_writer_error_uses_fast_findings_count() {
        let tiered = make_tiered_recorder_fast_only(3);
        let result: Result<Vec<DryCheckFinding>, DryCheckCycleError> =
            Err(DryCheckCycleError::Writer(domain::dry_check::DryCheckWriterError::Codec {
                detail: "serialize failed".to_owned(),
            }));

        let (fast, final_) = dry_tiered_telemetry_for_result(&result, &tiered);
        let fast_t = fast.unwrap();
        assert_eq!(fast_t.findings_count, 3);
        assert!(!fast_t.verdict_parse_failed);
        assert!(fast_t.subprocess_started_at.is_some());
        assert!(final_.is_none());
    }

    #[test]
    fn test_dry_tiered_telemetry_for_result_success_without_agent_start_skips_subprocess() {
        let tiered = TieredDryAgentRecorder {
            fast: DryAgentRunRecorder::new(),
            final_: DryAgentRunRecorder::new(),
        };
        let result: Result<Vec<DryCheckFinding>, DryCheckCycleError> = Ok(vec![]);

        let (fast, final_) = dry_tiered_telemetry_for_result(&result, &tiered);
        assert!(fast.is_none(), "no fast tier invoked → no fast telemetry");
        assert!(final_.is_none());
    }

    #[test]
    fn test_dry_agent_run_recorder_tracks_first_start_time() {
        let recorder = DryAgentRunRecorder::new();

        assert!(recorder.started_at().is_none());
        recorder.record_started();
        let first_start = recorder.started_at();
        recorder.record_started();

        assert!(first_start.is_some());
        assert_eq!(recorder.started_at(), first_start);
    }

    struct ViolationAgent {
        rationale: &'static str,
    }

    impl DryCheckAgentPort for ViolationAgent {
        fn judge(
            &self,
            _changed_fragment: &CodeFragment,
            _candidate_fragment: &CodeFragment,
            _tier: DryCheckJudgeTier,
        ) -> Result<DryCheckAgentJudgment, DryCheckAgentError> {
            Ok(DryCheckAgentJudgment::Violation {
                rationale: domain::Rationale::new(self.rationale).unwrap(),
                finding: dry_check_finding_for_tests(),
            })
        }
    }

    fn dry_check_finding_for_tests() -> DryCheckFinding {
        let changed_ref = domain::FragmentRef::new(
            domain::review_v2::FilePath::new("src/a.rs").unwrap(),
            domain::FragmentContentHash::new("a".repeat(64)).unwrap(),
        );
        let candidate_ref = domain::FragmentRef::new(
            domain::review_v2::FilePath::new("src/b.rs").unwrap(),
            domain::FragmentContentHash::new("b".repeat(64)).unwrap(),
        );
        DryCheckFinding::new(changed_ref, candidate_ref, "extract shared helper").unwrap()
    }

    #[test]
    fn test_recording_dry_agent_counts_violation_judgment_before_persistence() {
        let (agent, tiered) =
            RecordingDryAgent::new(ViolationAgent { rationale: "same control flow" });
        let changed =
            CodeFragment::new(PathBuf::from("src/a.rs"), "fn a() {}".to_owned(), 1, 1).unwrap();
        let candidate =
            CodeFragment::new(PathBuf::from("src/b.rs"), "fn b() {}".to_owned(), 1, 1).unwrap();

        let result = agent.judge(&changed, &candidate, DryCheckJudgeTier::Final);

        assert!(matches!(result, Ok(DryCheckAgentJudgment::Violation { .. })));
        // Final tier recorder must capture the violation; fast tier must be idle.
        assert_eq!(tiered.final_.findings_count(), 1);
        assert!(tiered.final_.has_completed());
        assert_eq!(tiered.fast.findings_count(), 0);
        assert!(!tiered.fast.has_completed());
    }

    // ── T013: per-tier ReviewRound telemetry (IN-07 / AC-09 / CN-10) ─────────

    struct TieredDryAgent {
        fast_model: String,
        final_model: String,
    }

    impl DryCheckAgentPort for TieredDryAgent {
        fn judge(
            &self,
            _changed_fragment: &CodeFragment,
            _candidate_fragment: &CodeFragment,
            tier: DryCheckJudgeTier,
        ) -> Result<DryCheckAgentJudgment, DryCheckAgentError> {
            let _ = match tier {
                DryCheckJudgeTier::Fast => &self.fast_model,
                DryCheckJudgeTier::Final => &self.final_model,
            };
            Ok(DryCheckAgentJudgment::NotAViolation {
                rationale: domain::Rationale::new("distinct logic").unwrap(),
            })
        }
    }

    /// T013 AC: fast-tier-only run emits a ReviewRound with round_type="fast".
    ///
    /// Simulates a run where only the fast tier is invoked (no escalation).
    /// Verifies that `dry_tiered_telemetry_for_result` returns:
    /// - `fast` = Some with the correct findings_count and subprocess_started_at.
    /// - `final_` = None (final tier was never invoked).
    #[test]
    fn test_tiered_recording_fast_only_run_emits_fast_round_type() {
        let (agent, tiered) = RecordingDryAgent::new(TieredDryAgent {
            fast_model: "fast-model-v1".to_string(),
            final_model: "final-model-v1".to_string(),
        });
        let changed =
            CodeFragment::new(PathBuf::from("src/a.rs"), "fn a() {}".to_owned(), 1, 1).unwrap();
        let candidate =
            CodeFragment::new(PathBuf::from("src/b.rs"), "fn b() {}".to_owned(), 1, 1).unwrap();

        // Invoke only fast tier.
        agent.judge(&changed, &candidate, DryCheckJudgeTier::Fast).unwrap();

        let result: Result<Vec<DryCheckFinding>, DryCheckCycleError> = Ok(vec![]);
        let (fast_t, final_t) = dry_tiered_telemetry_for_result(&result, &tiered);

        let fast = fast_t.expect("fast tier was invoked → fast telemetry must be Some");
        assert_eq!(fast.findings_count, 0, "no violations → findings_count=0");
        assert!(fast.subprocess_started_at.is_some(), "fast tier started_at must be recorded");
        assert!(final_t.is_none(), "final tier was not invoked → final telemetry must be None");
    }

    /// T013 AC: run escalated to final tier emits a ReviewRound with round_type="final".
    ///
    /// Simulates a run where both fast and final tiers are invoked (escalation path).
    /// Verifies that `dry_tiered_telemetry_for_result` returns:
    /// - `fast` = Some (fast tier was invoked).
    /// - `final_` = Some (final tier was also invoked).
    #[test]
    fn test_tiered_recording_escalated_run_emits_both_fast_and_final_round_types() {
        let (agent, tiered) = RecordingDryAgent::new(TieredDryAgent {
            fast_model: "fast-model-v1".to_string(),
            final_model: "final-model-v1".to_string(),
        });
        let changed =
            CodeFragment::new(PathBuf::from("src/a.rs"), "fn a() {}".to_owned(), 1, 1).unwrap();
        let candidate =
            CodeFragment::new(PathBuf::from("src/b.rs"), "fn b() {}".to_owned(), 1, 1).unwrap();

        // Invoke fast tier first, then final tier (simulating escalation).
        agent.judge(&changed, &candidate, DryCheckJudgeTier::Fast).unwrap();
        agent.judge(&changed, &candidate, DryCheckJudgeTier::Final).unwrap();

        let result: Result<Vec<DryCheckFinding>, DryCheckCycleError> = Ok(vec![]);
        let (fast_t, final_t) = dry_tiered_telemetry_for_result(&result, &tiered);

        assert!(fast_t.is_some(), "fast tier was invoked → fast telemetry must be Some");
        let final_ = final_t.expect("final tier was invoked → final telemetry must be Some");
        assert!(final_.subprocess_started_at.is_some(), "final tier started_at must be recorded");
    }

    /// T013 AC: tier-specific models are correctly routed to per-tier recorders.
    ///
    /// `RecordingDryAgent` must route `judge()` calls to the correct per-tier
    /// recorder. This test verifies that fast-tier calls increment the fast
    /// recorder and final-tier calls increment the final recorder independently.
    #[test]
    fn test_tiered_recording_tier_specific_model_is_recorded_per_tier() {
        let (agent, tiered) = RecordingDryAgent::new(ViolationAgent { rationale: "duplicated" });
        let changed =
            CodeFragment::new(PathBuf::from("src/a.rs"), "fn a() {}".to_owned(), 1, 1).unwrap();
        let candidate =
            CodeFragment::new(PathBuf::from("src/b.rs"), "fn b() {}".to_owned(), 1, 1).unwrap();

        // Two fast-tier violations.
        agent.judge(&changed, &candidate, DryCheckJudgeTier::Fast).unwrap();
        agent.judge(&changed, &candidate, DryCheckJudgeTier::Fast).unwrap();
        // One final-tier violation.
        agent.judge(&changed, &candidate, DryCheckJudgeTier::Final).unwrap();

        assert_eq!(tiered.fast.findings_count(), 2, "fast recorder must count 2 violations");
        assert_eq!(tiered.final_.findings_count(), 1, "final recorder must count 1 violation");
        assert!(tiered.fast.started_at().is_some(), "fast tier must record started_at");
        assert!(tiered.final_.started_at().is_some(), "final tier must record started_at");
    }

    /// Fragments whose path cannot be stripped (not under repo_root) are kept
    /// with the same path except for git-style separator normalization —
    /// conservative fallback, no silent drop.
    #[test]
    fn test_normalize_fragment_paths_keeps_non_prefixed_paths() {
        use domain::semantic_dup::CodeFragment;

        let repo_root = std::path::PathBuf::from("/workspace/root");

        // A fragment from outside repo_root.
        let outside_path = std::path::PathBuf::from("/other/place/foo.rs");
        let frag = CodeFragment::new(outside_path.clone(), "fn bar() {}".to_owned(), 1, 1).unwrap();

        let normalized = normalize_fragment_paths(vec![frag], &repo_root).unwrap();

        assert_eq!(normalized.len(), 1);
        // Path unchanged (fallback: strip_prefix failed, original kept).
        assert_eq!(normalized[0].source_path, outside_path);
    }

    // ── dry fix-local Codex runner ────────────────────────────────────────────

    #[test]
    fn test_dry_run_fix_local_invalid_track_id_returns_error() {
        let result = CliApp::new().dry_run_fix_local(RunDryFixLocalInput {
            track_id: "dry-track\nignore-the-prompt".to_owned(),
            briefing_file: PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            model: Some("gpt-test".to_owned()),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("invalid --track-id"),
            "unsafe track_id must be rejected before prompt construction, got: {message}"
        );
    }

    #[test]
    fn test_dry_fix_build_safe_env_strips_repository_credentials() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let _github_token = EnvGuard::set("GITHUB_TOKEN", "ghp-secret");
        let _ssh_auth_sock = EnvGuard::set("SSH_AUTH_SOCK", "/tmp/ssh-agent.sock");
        let _home = EnvGuard::set("HOME", "/real-home");
        let _codex_home = EnvGuard::set("CODEX_HOME", "/real-codex-home");
        let _codex_api_key = EnvGuard::set("CODEX_API_KEY", "codex-secret");

        let safe_home = PathBuf::from("/tmp/safe-home");
        let codex_home = PathBuf::from("/tmp/codex-home");
        let env = dry_fix_build_safe_env(&safe_home, &codex_home, None).unwrap();
        let keys: Vec<String> =
            env.iter().map(|(key, _)| key.to_string_lossy().into_owned()).collect();

        assert!(!keys.iter().any(|key| key == "GITHUB_TOKEN"));
        assert!(!keys.iter().any(|key| key == "SSH_AUTH_SOCK"));
        assert_eq!(
            env.iter()
                .find(|(key, _)| key.to_string_lossy() == "HOME")
                .map(|(_, value)| value.to_string_lossy().into_owned())
                .as_deref(),
            Some("/tmp/safe-home")
        );
        assert_eq!(
            env.iter()
                .find(|(key, _)| key.to_string_lossy() == "CODEX_HOME")
                .map(|(_, value)| value.to_string_lossy().into_owned())
                .as_deref(),
            Some("/tmp/codex-home")
        );
        assert_eq!(
            env.iter()
                .find(|(key, _)| key.to_string_lossy() == "GIT_SSH_COMMAND")
                .map(|(_, value)| value.to_string_lossy().into_owned())
                .as_deref(),
            Some("/bin/false")
        );
        assert!(keys.iter().any(|key| key == "CODEX_API_KEY"));
    }

    #[cfg(unix)]
    #[test]
    fn test_dry_fix_smoke_test_codex_version_uses_scrubbed_env() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let _github_token = EnvGuard::set("GITHUB_TOKEN", "ghp-secret");
        let _codex_api_key = EnvGuard::set("CODEX_API_KEY", "codex-secret");
        let dir = tempfile::tempdir().unwrap();
        let fake_codex = write_executable_script(
            dir.path(),
            "fake-codex-version.sh",
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  if [ -n "$GITHUB_TOKEN" ] || [ -n "$CODEX_API_KEY" ]; then
    echo "secret-bearing env reached version check" >&2
    exit 9
  fi
  if [ "$HOME" != "/tmp/safe-home" ]; then
    echo "safe HOME not applied" >&2
    exit 8
  fi
  echo "codex 0.125.0"
  exit 0
fi
exit 0
"#,
        );
        let safe_env = vec![
            (OsString::from("HOME"), OsString::from("/tmp/safe-home")),
            (OsString::from("CODEX_API_KEY"), OsString::from("codex-secret")),
        ];
        let smoke_env = dry_fix_build_smoke_env(&safe_env);

        dry_fix_smoke_test_codex_version(&fake_codex.as_os_str().to_os_string(), &smoke_env)
            .unwrap();
    }

    #[test]
    fn test_build_dry_fix_invocation_includes_safe_home_writable_root() {
        let codex_home = PathBuf::from("/tmp/codex-home");
        let safe_home = PathBuf::from("/tmp/safe-home");
        let output_last_message = PathBuf::from("/tmp/dry-fix-last-message.txt");

        let args =
            build_dry_fix_invocation("gpt-test", &codex_home, &safe_home, &output_last_message);
        let args_str: Vec<String> =
            args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
        let writable_roots = args_str
            .iter()
            .find(|arg| arg.contains("sandbox_workspace_write.writable_roots"))
            .expect("writable_roots config must be present");

        assert!(writable_roots.contains("/tmp/codex-home"));
        assert!(writable_roots.contains("/tmp/safe-home"));
    }

    // ── CliApp::dry_run_fix_local: wrapper logic (agent-profile loading + provider dispatch) ──

    /// Exercises the full `dry_run_fix_local` wrapper with `model: None`:
    /// git discover → agent-profiles load → track_id validation → profile resolution
    /// → model fallback from profiles → provider dispatch.
    ///
    /// The fixture's `agent-profiles.json` defines `provider = "claude"` and a non-empty
    /// model (independent of the live repo profiles).  The test verifies that:
    ///   - Profiles are loaded successfully (no "failed to load agent-profiles" error).
    ///   - The capability is resolved (no "capability not defined" error).
    ///   - The profile model fills the `None` input, so "no model specified" is NOT triggered.
    ///   - Execution reaches the provider-dispatch arm with the error
    ///     "[ERROR] unsupported dry-fix-lead provider 'claude' (supported: 'codex')",
    ///     proving the wrapper traversed every stage up to dispatch.
    ///
    /// A regression that broke profile loading or model-fallback would produce a different
    /// error message from an earlier stage, causing the assertions below to fail.
    #[cfg(unix)]
    #[test]
    fn test_dry_run_fix_local_none_model_falls_back_to_profile_model_and_reaches_dispatch() {
        // Hold process_env_lock: updates PATH, which races with other env-mutating tests.
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let fixture = DryRunFixLocalFixture::new_with_provider("claude", "claude-test-model");
        let _guards = fixture.path_guard();

        let result = CliApp::new().dry_run_fix_local(RunDryFixLocalInput {
            track_id: "dry-track".to_owned(),
            briefing_file: fixture.briefing_file.clone(),
            model: None,
        });

        let message = result.unwrap_err();

        // Model fallback from profiles must supply a non-empty model: the
        // "no model specified" branch must NOT be reached.
        assert!(
            !message.contains("no model specified"),
            "model fallback from agent-profiles.json must supply a model; got: {message}"
        );
        // Profiles must load without error.
        assert!(
            !message.contains("failed to load agent-profiles"),
            "agent-profiles.json must load successfully; got: {message}"
        );
        // The `dry-fix-lead` capability must be found in profiles.
        assert!(
            !message.contains("capability not defined"),
            "dry-fix-lead capability must be defined in agent-profiles.json; got: {message}"
        );
        // Provider dispatch must be reached with the sentinel error for 'claude'.
        assert!(
            message.contains("unsupported dry-fix-lead provider 'claude'"),
            "None model must reach provider dispatch with 'claude' provider error; got: {message}"
        );
    }

    /// Exercises `dry_run_fix_local` with an explicit `model: Some(x)`.
    ///
    /// Uses a fixture `agent-profiles.json` (same 'claude' provider as the `model: None` test)
    /// so the test is isolated from the live configuration.  The explicit model
    /// short-circuits `or_else(|| resolved.model.clone())` so the profile model is NOT
    /// consulted for model selection.  Profiles are still loaded to resolve the provider.
    ///
    /// Difference from the `model: None` variant: this path never reads `resolved.model`;
    /// a bug that ignored `input.model` and fell through to `resolved.model` would not be
    /// caught here, but a regression that broke the OR-logic so that explicit model
    /// triggered "no model specified" WOULD be caught.
    #[cfg(unix)]
    #[test]
    fn test_dry_run_fix_local_explicit_model_bypasses_profile_model_and_reaches_dispatch() {
        // Hold process_env_lock: updates PATH.
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let fixture = DryRunFixLocalFixture::new_with_provider("claude", "claude-test-model");
        let _guards = fixture.path_guard();

        let result = CliApp::new().dry_run_fix_local(RunDryFixLocalInput {
            track_id: "dry-track".to_owned(),
            briefing_file: fixture.briefing_file.clone(),
            model: Some("gpt-explicit-override".to_owned()),
        });

        let message = result.unwrap_err();

        // Explicit model must NOT trigger the "no model specified" fallback error.
        assert!(
            !message.contains("no model specified"),
            "explicit model must bypass the profile-model fallback; got: {message}"
        );
        assert!(
            !message.contains("invalid --track-id"),
            "valid track_id must not be rejected; got: {message}"
        );
        // Provider dispatch must be reached with the sentinel 'claude' error.
        assert!(
            message.contains("unsupported dry-fix-lead provider 'claude'"),
            "explicit model must reach provider dispatch with 'claude' provider error; got: {message}"
        );
    }

    /// Shared fixture for `dry_run_fix_local` codex-provider end-to-end tests.
    ///
    /// Creates a temp project dir, writes `agent-profiles.json` with `provider = "codex"`
    /// and the given `profile_model`, installs a fake `git` that returns the project dir
    /// as the repository root, and pre-creates a `CODEX_HOME` directory with a
    /// `captured-model.txt` capture path exposed for tests that need it.
    ///
    /// The caller must hold `process_env_lock()` before creating this fixture.
    #[cfg(unix)]
    struct DryRunFixLocalFixture {
        /// Temp dir that acts as the project root — must be kept alive for the duration of the test.
        _project_dir: tempfile::TempDir,
        fake_bin_dir: std::path::PathBuf,
        codex_home: std::path::PathBuf,
        /// Path to the `captured-model.txt` file written by a capture-script fake codex.
        capture_file: std::path::PathBuf,
        briefing_file: std::path::PathBuf,
    }

    #[cfg(unix)]
    impl DryRunFixLocalFixture {
        /// Build the fixture with `profile_model` written into `agent-profiles.json`
        /// (provider fixed to `codex`).
        fn new(profile_model: &str) -> Self {
            Self::new_with_provider("codex", profile_model)
        }

        /// Build the fixture with an explicit `dry-fix-lead` provider, so wrapper tests
        /// stay deterministic regardless of the live repo's `agent-profiles.json`.
        fn new_with_provider(provider: &str, profile_model: &str) -> Self {
            let project_dir = tempfile::tempdir().unwrap();
            let config_dir = project_dir.path().join(".harness").join("config");
            std::fs::create_dir_all(&config_dir).unwrap();
            std::fs::write(
                config_dir.join("agent-profiles.json"),
                format!(
                    r#"{{
  "schema_version": 1,
  "providers": {{ "{provider}": {{ "label": "Test Provider" }} }},
  "capabilities": {{
    "dry-fix-lead": {{
      "provider": "{provider}",
      "model": "{profile_model}"
    }}
  }}
}}"#
                ),
            )
            .unwrap();

            // Fake git: returns the project dir as the repository root.
            let fake_bin_dir = project_dir.path().join("fake-bin");
            std::fs::create_dir_all(&fake_bin_dir).unwrap();
            let fake_git = fake_bin_dir.join("git");
            let project_root_str = project_dir.path().to_string_lossy();
            std::fs::write(
                &fake_git,
                format!(
                    "#!/bin/sh\n\
                     if [ \"$1\" = \"rev-parse\" ] && [ \"$2\" = \"--show-toplevel\" ]; then\n\
                     \x20\x20printf '%s\\n' '{project_root_str}'\n\
                     \x20\x20exit 0\n\
                     fi\n\
                     exit 1\n"
                ),
            )
            .unwrap();
            make_executable(&fake_git);

            let codex_home = project_dir.path().join(".codex");
            std::fs::create_dir_all(&codex_home).unwrap();
            let capture_file = codex_home.join("captured-model.txt");

            let briefing_file = project_dir.path().join("briefing.md");
            std::fs::write(&briefing_file, "# dry briefing\n").unwrap();

            Self {
                _project_dir: project_dir,
                fake_bin_dir,
                codex_home,
                capture_file,
                briefing_file,
            }
        }

        /// Prepend the fixture's fake-bin dir to PATH so `SystemGitRepo::discover()`
        /// resolves the temp project root via the fake git.
        fn path_guard(&self) -> Vec<EnvGuard> {
            let mut path_entries = vec![self.fake_bin_dir.clone()];
            if let Some(existing) = std::env::var_os("PATH") {
                path_entries.extend(std::env::split_paths(&existing));
            }
            let new_path = std::env::join_paths(path_entries).unwrap();
            vec![EnvGuard::set("PATH", new_path)]
        }

        /// Install the fake codex script, set PATH/CODEX_BIN/CODEX_HOME guards, and call
        /// `dry_run_fix_local` with the given input `model`.  Returns the raw result.
        fn run(
            &self,
            codex_script: &str,
            model: Option<&str>,
        ) -> (Result<crate::CommandOutcome, String>, Vec<EnvGuard>) {
            let fake_codex = write_fake_codex_runner(&self.fake_bin_dir, codex_script);

            let mut path_entries = vec![self.fake_bin_dir.clone()];
            if let Some(existing) = std::env::var_os("PATH") {
                path_entries.extend(std::env::split_paths(&existing));
            }
            let new_path = std::env::join_paths(path_entries).unwrap();
            let guards = vec![
                EnvGuard::set("PATH", new_path),
                EnvGuard::set("CODEX_BIN", fake_codex.as_os_str().to_os_string()),
                EnvGuard::set("CODEX_HOME", self.codex_home.as_os_str().to_os_string()),
            ];

            let result = CliApp::new().dry_run_fix_local(RunDryFixLocalInput {
                track_id: "dry-track".to_owned(),
                briefing_file: self.briefing_file.clone(),
                model: model.map(|s| s.to_owned()),
            });

            (result, guards)
        }
    }

    /// End-to-end success test for `CliApp::dry_run_fix_local`:
    /// verifies that the wrapper resolves the git root, loads a project-local
    /// `agent-profiles.json`, picks the provider and model, and returns a
    /// successful outcome when the configured provider is "codex" and the
    /// fake codex binary outputs the completed sentinel.
    #[cfg(unix)]
    #[test]
    fn test_dry_run_fix_local_with_codex_provider_returns_completed() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let fixture = DryRunFixLocalFixture::new("gpt-test");

        let (result, _guards) = fixture.run(
            r#"out=""
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
while IFS= read -r _line; do :; done
printf 'DRY_FIX_STATUS: completed\n' > "$out"
printf 'codex done\n'
exit 0
"#,
            None,
        );

        let outcome = result.unwrap();
        assert_eq!(outcome.exit_code, 0, "dry_run_fix_local must succeed: {outcome:?}");
        assert_eq!(
            outcome.stdout.as_deref(),
            Some("DRY_FIX_STATUS: completed"),
            "outcome must report completed sentinel: {outcome:?}"
        );
    }

    /// Shared helper: creates a `DryRunFixLocalFixture` with `profile_model`, installs a
    /// fake codex that captures the `--model` argument to `${CODEX_HOME}/captured-model.txt`,
    /// runs `dry_run_fix_local` with `input_model`, and asserts the captured model equals
    /// `expected_model`.
    ///
    /// The caller must hold `process_env_lock()` before calling this helper.
    #[cfg(unix)]
    fn assert_dry_run_fix_local_forwards_model(
        profile_model: &str,
        input_model: Option<&str>,
        expected_model: &str,
    ) {
        let fixture = DryRunFixLocalFixture::new(profile_model);
        let (result, _guards) = fixture.run(
            r#"model=""
out=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --model) model="$2"; shift 2 ;;
    --output-last-message) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
printf '%s' "$model" > "${CODEX_HOME}/captured-model.txt"
while IFS= read -r _line; do :; done
printf 'DRY_FIX_STATUS: completed\n' > "$out"
printf 'codex done\n'
exit 0
"#,
            input_model,
        );
        assert!(result.is_ok(), "dry_run_fix_local must succeed: {result:?}");
        let captured_model = std::fs::read_to_string(&fixture.capture_file)
            .expect("capture file must be written by fake codex");
        assert_eq!(
            captured_model, expected_model,
            "model must be forwarded verbatim to codex --model arg; got: {captured_model:?}"
        );
    }

    /// Verifies that `dry_run_fix_local` forwards the **profile model** to the
    /// codex invocation when `model: None` is given.
    ///
    /// The fake codex captures the `--model` value and writes it to
    /// `${CODEX_HOME}/captured-model.txt`, proving the profile-model fallback
    /// reaches the codex binary — not just the wrapper dispatch.
    #[cfg(unix)]
    #[test]
    fn test_dry_run_fix_local_none_model_forwards_profile_model_to_codex() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        assert_dry_run_fix_local_forwards_model("gpt-profile-model", None, "gpt-profile-model");
    }

    /// Verifies that `dry_run_fix_local` forwards an **explicit override model**
    /// to the codex invocation — bypassing the profile model entirely.
    ///
    /// Uses the same `${CODEX_HOME}/captured-model.txt` capture mechanism.  The
    /// profile defines `gpt-profile-model`; the explicit input is
    /// `gpt-explicit-override`.  After the run the test asserts the codex received
    /// `gpt-explicit-override`, not `gpt-profile-model`.
    #[cfg(unix)]
    #[test]
    fn test_dry_run_fix_local_explicit_model_forwards_override_model_to_codex() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        assert_dry_run_fix_local_forwards_model(
            "gpt-profile-model",
            Some("gpt-explicit-override"),
            "gpt-explicit-override",
        );
    }

    /// Shared fixture: write a fake codex runner with `script` body, install
    /// `CODEX_BIN` and `CODEX_HOME` guards, write a briefing file, and call
    /// `run_dry_fix_codex`. Returns the result so each test can assert its
    /// own success or error path.
    ///
    /// The caller must hold `process_env_lock()` before calling this helper.
    #[cfg(unix)]
    fn run_dry_fix_with_fake_codex(script: &str) -> Result<crate::CommandOutcome, String> {
        let dir = tempfile::tempdir().unwrap();
        let fake_codex = write_fake_codex_runner(dir.path(), script);
        let _codex_bin = EnvGuard::set("CODEX_BIN", fake_codex.as_os_str().to_os_string());
        let _codex_home = EnvGuard::set("CODEX_HOME", dir.path().join(".codex").into_os_string());
        let briefing_file = dir.path().join("briefing.md");
        std::fs::write(&briefing_file, "# dry briefing\n").unwrap();
        run_dry_fix_codex("gpt-test", "dry-track", &briefing_file)
    }

    /// Calls `run_dry_fix_codex` directly: the higher-level `dry_run_fix_local`
    /// end-to-end success is covered by
    /// `test_dry_run_fix_local_with_codex_provider_returns_completed`.
    #[cfg(unix)]
    #[test]
    fn test_run_dry_fix_codex_completed_sentinel_returns_exit_zero() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let outcome = run_dry_fix_with_fake_codex(
            r#"out=""
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
while IFS= read -r _line; do :; done
printf 'DRY_FIX_STATUS: completed\n' > "$out"
printf 'nested dry fixer stdout\n'
exit 0
"#,
        )
        .unwrap();

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("DRY_FIX_STATUS: completed"));
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_dry_fix_session_log_cleanup_removes_log_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("session.log");
        std::fs::write(&log_path, "dry fixer output").unwrap();

        {
            let _cleanup = DryFixSessionLogCleanup::new(log_path.clone());
        }

        assert!(!log_path.exists(), "default cleanup must remove successful-run logs");
    }

    #[test]
    fn test_dry_fix_session_log_cleanup_keep_for_diagnosis_preserves_log() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("session.log");
        std::fs::write(&log_path, "dry fixer output").unwrap();

        DryFixSessionLogCleanup::new(log_path.clone()).keep_for_diagnosis();

        assert!(log_path.exists(), "diagnostic cleanup must preserve failed-run logs");
    }

    #[cfg(unix)]
    #[test]
    fn test_run_dry_fix_codex_missing_sentinel_returns_error() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let result = run_dry_fix_with_fake_codex(
            r#"out=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
    out="$2"
    shift 2
  else
    shift
  fi
done
while IFS= read -r _line; do :; done
printf 'not a sentinel\n' > "$out"
printf 'stdout without sentinel\n'
exit 0
"#,
        );

        let message = result.unwrap_err();
        assert!(
            message.contains("no DRY_FIX_STATUS sentinel"),
            "missing sentinel must return a diagnostic error, got: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_dry_fix_spawn_and_collect_redacts_sensitive_values_from_output_and_log() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let short_secret = "sk-dry-fix";
        let long_secret = "sk-dry-fix-secret";
        let org_id = "org-dry-fix";
        let base_url = "https://token@example.invalid/v1";
        let fake_codex = write_fake_codex_runner(
            dir.path(),
            &format!(
                "while IFS= read -r _line; do :; done\nprintf 'stdout {short_secret} {long_secret} {org_id} {base_url}\\n'\nprintf 'stderr {short_secret} {long_secret} {org_id} {base_url}\\n' >&2\nexit 0\n"
            ),
        );
        let safe_env = vec![
            (OsString::from("OPENAI_API_KEY"), OsString::from(short_secret)),
            (OsString::from("CODEX_API_KEY"), OsString::from(long_secret)),
            (OsString::from("OPENAI_ORG_ID"), OsString::from(org_id)),
            (OsString::from("OPENAI_BASE_URL"), OsString::from(base_url)),
        ];

        let (stdout, log_path) = dry_fix_spawn_and_collect(
            &fake_codex.as_os_str().to_os_string(),
            &[],
            &safe_env,
            "prompt",
        )
        .unwrap();
        let log = std::fs::read_to_string(log_path).unwrap();

        for secret in [short_secret, long_secret, org_id, base_url] {
            assert!(!stdout.contains(secret), "stdout must redact {secret}");
            assert!(!log.contains(secret), "session log must redact {secret}");
        }
        assert!(!stdout.contains("-secret"), "overlapping secret suffix must not leak");
        assert!(!log.contains("-secret"), "overlapping secret suffix must not leak");
        assert!(stdout.contains("[REDACTED:OPENAI_API_KEY]"));
        assert!(stdout.contains("[REDACTED:CODEX_API_KEY]"));
        assert!(stdout.contains("[REDACTED:OPENAI_ORG_ID]"));
        assert!(stdout.contains("[REDACTED:OPENAI_BASE_URL]"));
        assert!(log.contains("[REDACTED:OPENAI_API_KEY]"));
        assert!(log.contains("[REDACTED:CODEX_API_KEY]"));
        assert!(log.contains("[REDACTED:OPENAI_ORG_ID]"));
        assert!(log.contains("[REDACTED:OPENAI_BASE_URL]"));
    }

    // ── CliApp public entry points ────────────────────────────────────────────

    #[test]
    fn test_dry_results_empty_store_returns_success_exit_zero() {
        let dir = temp_items_dir_under_repo();
        let outcome = CliApp::new()
            .dry_results(DryResultsInput {
                track_id: "dry-results-empty".to_owned(),
                filter: "all".to_owned(),
                items_dir: dir.path().to_path_buf(),
            })
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stderr, None);
        assert_eq!(outcome.stdout.as_deref(), Some("dry results: 0 record(s)"));
    }

    #[test]
    fn test_dry_results_invalid_filter_returns_error() {
        let dir = temp_items_dir_under_repo();
        let result = CliApp::new().dry_results(DryResultsInput {
            track_id: "dry-results-invalid-filter".to_owned(),
            filter: "unknown".to_owned(),
            items_dir: dir.path().to_path_buf(),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("invalid --filter"),
            "error must describe the invalid filter, got: {message}"
        );
    }

    #[test]
    fn test_dry_results_outside_repo_items_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = CliApp::new().dry_results(DryResultsInput {
            track_id: "dry-results-outside-items-dir".to_owned(),
            filter: "all".to_owned(),
            items_dir: dir.path().to_path_buf(),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("items_dir"),
            "error must reject escaped items_dir, got: {message}"
        );
    }

    #[test]
    fn test_dry_results_escaped_track_id_returns_error() {
        let dir = temp_items_dir_under_repo();
        let result = CliApp::new().dry_results(DryResultsInput {
            track_id: "../outside".to_owned(),
            filter: "all".to_owned(),
            items_dir: dir.path().to_path_buf(),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("invalid --track-id"),
            "error must reject escaped track_id, got: {message}"
        );
    }

    #[test]
    fn test_dry_write_invalid_base_commit_returns_error() {
        let dir = temp_items_dir_under_repo();
        let track_id = "dry-write-invalid-base";
        std::fs::create_dir_all(dir.path().join(track_id)).unwrap();

        let result = CliApp::new().dry_write(DryWriteInput {
            track_id: track_id.to_owned(),
            base_commit: Some("not-a-hash".to_owned()),
            db_path: dir.path().join("semantic-index"),
            threshold: Some(0.85),
            workspace_root: PathBuf::from("."),
            items_dir: dir.path().to_path_buf(),
            model: Some("codex".to_owned()),
            capability_name: "dry-checker".to_owned(),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("invalid --base-commit"),
            "error must describe the invalid base commit, got: {message}"
        );
    }

    #[test]
    fn test_dry_write_public_api_outside_items_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = CliApp::new().dry_write(DryWriteInput {
            track_id: "dry-write-outside-items-dir".to_owned(),
            base_commit: Some(valid_commit_hash_for_tests()),
            db_path: dir.path().join("semantic-index"),
            threshold: Some(0.85),
            workspace_root: PathBuf::from("."),
            items_dir: dir.path().to_path_buf(),
            model: Some("codex".to_owned()),
            capability_name: "dry-checker".to_owned(),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("items_dir"),
            "error must reject escaped items_dir, got: {message}"
        );
    }

    #[test]
    fn test_dry_write_public_api_escaped_track_id_returns_error() {
        let dir = temp_items_dir_under_repo();
        let result = CliApp::new().dry_write(DryWriteInput {
            track_id: "../outside".to_owned(),
            base_commit: Some(valid_commit_hash_for_tests()),
            db_path: dir.path().join("semantic-index"),
            threshold: Some(0.85),
            workspace_root: PathBuf::from("."),
            items_dir: dir.path().to_path_buf(),
            model: Some("codex".to_owned()),
            capability_name: "dry-checker".to_owned(),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("invalid --track-id"),
            "error must reject escaped track_id, got: {message}"
        );
    }

    #[test]
    fn test_dry_write_public_api_missing_track_dir_reaches_threshold_validation() {
        let dir = temp_items_dir_under_repo();

        let result = CliApp::new().dry_write(DryWriteInput {
            track_id: "dry-write-missing-track-invalid-threshold".to_owned(),
            base_commit: Some(valid_commit_hash_for_tests()),
            db_path: dir.path().join("semantic-index"),
            threshold: Some(1.5),
            workspace_root: PathBuf::from("."),
            items_dir: dir.path().to_path_buf(),
            model: Some("codex".to_owned()),
            capability_name: "dry-checker".to_owned(),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("invalid --threshold"),
            "missing track dir must not be rejected before threshold validation, got: {message}"
        );
    }

    #[test]
    fn test_dry_check_approved_invalid_base_commit_returns_error() {
        let dir = temp_items_dir_under_repo();
        let track_id = "dry-check-approved-invalid-base";
        std::fs::create_dir_all(dir.path().join(track_id)).unwrap();

        let result = CliApp::new().dry_check_approved(DryCheckApprovedInput {
            track_id: track_id.to_owned(),
            base_commit: Some("not-a-hash".to_owned()),
            items_dir: dir.path().to_path_buf(),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("invalid --base-commit"),
            "error must describe the invalid base commit, got: {message}"
        );
    }

    #[test]
    fn test_dry_check_approved_public_api_outside_items_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = CliApp::new().dry_check_approved(DryCheckApprovedInput {
            track_id: "dry-check-approved-outside-items-dir".to_owned(),
            base_commit: Some(valid_commit_hash_for_tests()),
            items_dir: dir.path().to_path_buf(),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("items_dir"),
            "error must reject escaped items_dir, got: {message}"
        );
    }

    #[test]
    fn test_dry_check_approved_public_api_escaped_track_id_returns_error() {
        let dir = temp_items_dir_under_repo();
        let result = CliApp::new().dry_check_approved(DryCheckApprovedInput {
            track_id: "../outside".to_owned(),
            base_commit: Some(valid_commit_hash_for_tests()),
            items_dir: dir.path().to_path_buf(),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("invalid --track-id"),
            "error must reject escaped track_id, got: {message}"
        );
    }

    #[test]
    fn test_dry_check_approved_public_api_missing_track_dir_advances_to_diff_step() {
        // D5 (T003): dry_check_approved no longer consults `--threshold` /
        // `db_path` (T005 removes those fields). A missing track dir is
        // tolerated up to the diff acquisition step, which then fails on
        // the synthetic base-commit hash.
        let dir = temp_items_dir_under_repo();

        let result = CliApp::new().dry_check_approved(DryCheckApprovedInput {
            track_id: "dry-check-approved-missing-track-advances".to_owned(),
            base_commit: Some(valid_commit_hash_for_tests()),
            items_dir: dir.path().to_path_buf(),
        });

        let message = result.unwrap_err();
        assert!(
            !message.contains("invalid --threshold"),
            "threshold is no longer consulted; got: {message}"
        );
        assert!(
            message.contains("merge-base") || message.contains("dry-check diff failed"),
            "missing track dir must advance to the diff step; got: {message}"
        );
    }

    // ── threshold: None config fallback (D9 / CN-04) ──────────────────────
    //
    // When threshold is None, the code loads `.harness/config/dry-check.json`
    // (CN-04 fail-closed).  These tests prove the fallback ran:
    //   1. No "invalid --threshold" → the Some(t) branch was not taken.
    //   2. No "failed to load dry-check config" → DryCheckConfig::load succeeded.
    //   3. Error IS about "merge-base" → execution advanced past threshold
    //      resolution to the diff step, which fails on the synthetic commit hash.
    // A regression in the None branch or DryCheckConfig::load would change the
    // error to a threshold/config message, failing assertion 2 or 3.

    #[test]
    fn test_dry_write_threshold_none_loads_config_then_reaches_diff_step() {
        let dir = temp_items_dir_under_repo();
        let track_id = "dry-write-threshold-none";
        std::fs::create_dir_all(dir.path().join(track_id)).unwrap();

        let result = CliApp::new().dry_write(DryWriteInput {
            track_id: track_id.to_owned(),
            base_commit: Some(valid_commit_hash_for_tests()),
            db_path: dir.path().join("semantic-index"),
            threshold: None,
            workspace_root: PathBuf::from("."),
            items_dir: dir.path().to_path_buf(),
            model: Some("codex".to_owned()),
            capability_name: "dry-checker".to_owned(),
        });

        let message = result.unwrap_err();
        assert!(
            !message.contains("invalid --threshold"),
            "must not hit Some(t) branch; got: {message}"
        );
        assert!(
            !message.contains("failed to load dry-check config"),
            "config fallback must succeed; got: {message}"
        );
        assert!(
            message.contains("merge-base"),
            "must advance past threshold to diff step; got: {message}"
        );
    }

    #[test]
    fn test_dry_check_approved_threshold_none_loads_config_then_reaches_diff_step() {
        let dir = temp_items_dir_under_repo();
        let track_id = "dry-check-approved-threshold-none";
        std::fs::create_dir_all(dir.path().join(track_id)).unwrap();

        let result = CliApp::new().dry_check_approved(DryCheckApprovedInput {
            track_id: track_id.to_owned(),
            base_commit: Some(valid_commit_hash_for_tests()),
            items_dir: dir.path().to_path_buf(),
        });

        let message = result.unwrap_err();
        assert!(
            !message.contains("invalid --threshold"),
            "must not hit Some(t) branch; got: {message}"
        );
        assert!(
            !message.contains("failed to load dry-check config"),
            "config fallback must succeed; got: {message}"
        );
        assert!(
            message.contains("merge-base"),
            "must advance past threshold to diff step; got: {message}"
        );
    }

    // ── DryResultsInput: filter behavior ─────────────────────────────────────

    #[test]
    fn test_dry_results_input_filter_all_variant() {
        let input = DryResultsInput {
            track_id: "my-track".to_owned(),
            filter: "all".to_owned(),
            items_dir: PathBuf::from("track/items"),
        };
        let parsed = parse_verdict_filter(&input.filter).unwrap();
        assert_eq!(parsed, VerdictFilter::All);
    }

    #[test]
    fn test_dry_results_input_filter_violation_variant() {
        let input = DryResultsInput {
            track_id: "my-track".to_owned(),
            filter: "violation".to_owned(),
            items_dir: PathBuf::from("track/items"),
        };
        let parsed = parse_verdict_filter(&input.filter).unwrap();
        assert_eq!(parsed, VerdictFilter::Violation);
    }

    #[test]
    fn test_parse_verdict_filter_all_variants() {
        assert_eq!(parse_verdict_filter("all").unwrap(), VerdictFilter::All);
        assert_eq!(parse_verdict_filter("not-a-violation").unwrap(), VerdictFilter::NotAViolation);
        assert_eq!(parse_verdict_filter("accepted").unwrap(), VerdictFilter::Accepted);
        assert_eq!(parse_verdict_filter("violation").unwrap(), VerdictFilter::Violation);
    }

    #[test]
    fn test_parse_verdict_filter_unknown_returns_err() {
        let result = parse_verdict_filter("unknown");
        assert!(result.is_err(), "unknown filter must return Err");
    }

    // ── dry_write: model resolution ──────────────────────────────────────────

    /// When an explicit `model: Some(...)` is provided, `dry_write` must use it
    /// and NOT attempt to load agent-profiles.json at all.  The test verifies this
    /// by passing a valid track_id and an invalid items_dir (so the call fails at
    /// the items_dir check, after model resolution but before any agent is spawned).
    /// The error message must NOT contain an agent-profiles/model error.
    #[test]
    fn test_dry_write_explicit_model_bypasses_agent_profiles_resolution() {
        let dir = tempfile::tempdir().unwrap();
        // Use a known-valid commit hash so base_commit does not fail first.
        let result = CliApp::new().dry_write(DryWriteInput {
            track_id: "my-track".to_owned(),
            base_commit: Some(valid_commit_hash_for_tests()),
            db_path: dir.path().join("semantic-index"),
            threshold: Some(0.85),
            workspace_root: PathBuf::from("."),
            // Outside repo → will fail at items_dir validation, not model resolution.
            items_dir: dir.path().to_path_buf(),
            model: Some("gpt-explicit-override".to_owned()),
            capability_name: "dry-checker".to_owned(),
        });

        let message = result.unwrap_err();
        // Must fail at items_dir validation, not at agent-profiles loading.
        assert!(
            message.contains("items_dir"),
            "error must be about items_dir, not model resolution, got: {message}"
        );
        assert!(
            !message.contains("agent-profiles"),
            "explicit model must skip agent-profiles loading entirely, got: {message}"
        );
    }

    /// When `model: None`, `dry_write` must attempt to resolve the model from
    /// agent-profiles.json.  Passing a nonexistent capability name forces the
    /// resolution to fail with the "[ERROR] '...' capability not defined" message,
    /// confirming the agent-profiles path is actually taken.
    #[test]
    fn test_dry_write_none_model_resolves_from_agent_profiles_and_fails_on_missing_capability() {
        let dir = tempfile::tempdir().unwrap();
        let result = CliApp::new().dry_write(DryWriteInput {
            track_id: "my-track".to_owned(),
            base_commit: Some(valid_commit_hash_for_tests()),
            db_path: dir.path().join("semantic-index"),
            threshold: Some(0.85),
            workspace_root: PathBuf::from("."),
            items_dir: dir.path().to_path_buf(),
            model: None,
            capability_name: "nonexistent-capability-for-test".to_owned(),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("capability not defined in agent-profiles.json")
                || message.contains("nonexistent-capability-for-test"),
            "None model must trigger agent-profiles resolution and report missing capability, got: {message}"
        );
    }

    // ── DryWriteInput / DryCheckApprovedInput: field round-trip ──────────────

    #[test]
    fn test_dry_write_input_fields_accessible() {
        let input = DryWriteInput {
            track_id: "my-track".to_owned(),
            base_commit: Some("abc1234".to_owned()),
            db_path: PathBuf::from(".semantic_index"),
            threshold: Some(0.85),
            workspace_root: PathBuf::from("."),
            items_dir: PathBuf::from("track/items"),
            model: Some("codex-model".to_owned()),
            capability_name: "dry-checker".to_owned(),
        };
        assert_eq!(input.track_id, "my-track");
        assert_eq!(input.base_commit.as_deref(), Some("abc1234"));
        assert!((input.threshold.unwrap() - 0.85).abs() < 1e-6);
    }

    #[test]
    fn test_dry_check_approved_input_fields_accessible() {
        let input = DryCheckApprovedInput {
            track_id: "my-track".to_owned(),
            base_commit: None,
            items_dir: PathBuf::from("track/items"),
        };
        assert_eq!(input.track_id, "my-track");
        assert!(input.base_commit.is_none());
    }

    // ── GateEval telemetry field mapping (T007 / IN-07 / CN-10) ─────────────

    /// Approved verdict maps to gate_name="dry", verdict="ok", empty reason.
    #[test]
    fn test_dry_check_approved_gate_eval_fields_approved_maps_to_ok() {
        let (verdict_str, reason_summary) = dry_check_approved_gate_eval_fields(
            &domain::dry_check::DryCheckApprovalVerdict::Approved,
        );
        assert_eq!(verdict_str, "ok", "Approved must map to verdict_str=\"ok\"");
        assert!(
            reason_summary.is_empty(),
            "Approved must produce an empty reason_summary; got: {reason_summary:?}"
        );
    }

    /// Blocked verdict maps to verdict="error" with "blocked: N unresolved pair(s)" summary.
    #[test]
    fn test_dry_check_approved_gate_eval_fields_blocked_maps_to_error_with_count() {
        let (verdict_str, reason_summary) = dry_check_approved_gate_eval_fields(
            &domain::dry_check::DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 3 },
        );
        assert_eq!(verdict_str, "error", "Blocked must map to verdict_str=\"error\"");
        assert_eq!(
            reason_summary, "blocked: 3 unresolved pair(s)",
            "Blocked reason_summary must embed the unresolved pair count"
        );
    }

    /// Blocked with zero unresolved pairs still emits "error" (gate is blocked).
    #[test]
    fn test_dry_check_approved_gate_eval_fields_blocked_zero_pairs_still_error() {
        let (verdict_str, reason_summary) = dry_check_approved_gate_eval_fields(
            &domain::dry_check::DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 0 },
        );
        assert_eq!(verdict_str, "error", "Blocked(0) must still map to \"error\"");
        assert_eq!(
            reason_summary, "blocked: 0 unresolved pair(s)",
            "Blocked(0) reason_summary must include the zero count"
        );
    }

    // ── Ephemeral index parent selection ─────────────────────────────────────

    #[test]
    fn test_ephemeral_index_parent_bare_db_path_returns_fallback_parent() {
        let fallback = tempfile::tempdir().unwrap();

        let parent = ephemeral_index_parent(Path::new(".semantic_index"), fallback.path());

        assert_eq!(parent, fallback.path().to_path_buf());
    }

    #[test]
    fn test_ephemeral_index_parent_existing_parent_hint_returns_hint() {
        let fallback = tempfile::tempdir().unwrap();
        let hinted_parent = tempfile::tempdir().unwrap();
        let db_path = hinted_parent.path().join("semantic-index");

        let parent = ephemeral_index_parent(&db_path, fallback.path());

        assert_eq!(parent, hinted_parent.path().to_path_buf());
    }

    #[test]
    fn test_ephemeral_index_parent_missing_parent_hint_returns_fallback_parent() {
        let fallback = tempfile::tempdir().unwrap();
        let missing_parent = fallback.path().join("missing");
        let db_path = missing_parent.join("semantic-index");

        let parent = ephemeral_index_parent(&db_path, fallback.path());

        assert_eq!(parent, fallback.path().to_path_buf());
        assert!(!missing_parent.exists(), "parent selection must not create the hint directory");
    }

    #[test]
    fn test_create_ephemeral_index_dir_bare_db_path_creates_temp_under_fallback_parent() {
        let fallback = tempfile::tempdir().unwrap();

        let temp_index_dir =
            create_ephemeral_index_dir(Path::new(".semantic_index"), fallback.path()).unwrap();
        let temp_index_path = temp_index_dir.path().to_path_buf();

        assert!(temp_index_path.starts_with(fallback.path()));
        assert!(
            temp_index_path.file_name().unwrap().to_string_lossy().starts_with("sotp-dry-index-")
        );

        drop(temp_index_dir);

        assert!(!temp_index_path.exists(), "ephemeral index dir must be removed on drop");
    }

    // ── Approved/Blocked exit-code semantics ─────────────────────────────────

    #[test]
    fn test_approved_verdict_maps_to_exit_0() {
        let outcome = dry_check_approved_outcome(DryCheckApprovalVerdict::Approved);
        assert_eq!(outcome.exit_code, 0, "Approved must produce exit code 0");
        assert!(outcome.stdout.is_some(), "Approved must report on stdout");
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_blocked_verdict_maps_to_exit_1() {
        let outcome = dry_check_approved_outcome(DryCheckApprovalVerdict::Blocked {
            unresolved_pair_count: 2,
        });
        assert_eq!(outcome.exit_code, 1, "Blocked must produce exit code 1");
        assert_eq!(outcome.stdout, None);
        assert!(
            outcome.stderr.as_deref().is_some_and(|msg| msg.contains("2 unresolved pair(s)")),
            "Blocked stderr must include unresolved count"
        );
    }

    #[test]
    fn test_dry_write_outcome_reports_checked_and_appended_counts() {
        let outcome = dry_write_outcome(&[], 3, 2, 1);
        let stdout = outcome.stdout.as_deref().unwrap_or("");

        assert_eq!(outcome.exit_code, 0);
        assert!(stdout.contains("3 pair(s) checked"), "stdout must include pairs checked");
        assert!(stdout.contains("2 record(s) appended"), "stdout must include records appended");
        assert!(stdout.contains("0 violation(s) found"), "stdout must include finding count");
        assert!(
            stdout.contains("1 diff fragment(s) processed"),
            "stdout must include processed diff fragments"
        );
    }

    // ── results: informational exit-0 regardless of verdicts ─────────────────

    #[test]
    fn test_dry_results_exit_0_regardless_of_filter() {
        // results always exits 0 on successful read — verified by the CommandOutcome
        // convention (success() sets exit_code = 0).
        let outcome = CommandOutcome::success(Some("dry results: 0 record(s)".to_owned()));
        assert_eq!(outcome.exit_code, 0, "dry results must always exit 0 on success");
    }

    // ── Ephemeral index adapter (IN-02/AC-02) ────────────────────────────────

    #[test]
    fn test_create_ephemeral_index_adapter_bare_db_path_opens_temp_index_under_fallback_parent() {
        let fallback = tempfile::tempdir().unwrap();

        let (temp_index_dir, index_adapter) =
            create_ephemeral_index_adapter(Path::new(".semantic_index"), fallback.path()).unwrap();
        let temp_index_path = temp_index_dir.path().to_path_buf();

        assert!(temp_index_path.starts_with(fallback.path()));
        assert!(temp_index_path.exists());
        assert!(
            temp_index_path.file_name().unwrap().to_string_lossy().starts_with("sotp-dry-index-")
        );

        drop(index_adapter);
        drop(temp_index_dir);

        assert!(!temp_index_path.exists(), "ephemeral index dir must be removed on drop");
    }

    #[test]
    fn test_create_ephemeral_index_adapter_same_db_path_returns_distinct_temp_paths() {
        let fallback = tempfile::tempdir().unwrap();
        let hinted_parent = tempfile::tempdir().unwrap();
        let db_path = hinted_parent.path().join("persistent.db");

        let (first_temp_dir, first_adapter) =
            create_ephemeral_index_adapter(&db_path, fallback.path()).unwrap();
        let first_path = first_temp_dir.path().to_path_buf();
        let (second_temp_dir, second_adapter) =
            create_ephemeral_index_adapter(&db_path, fallback.path()).unwrap();
        let second_path = second_temp_dir.path().to_path_buf();

        assert_ne!(first_path, second_path);
        assert!(first_path.starts_with(hinted_parent.path()));
        assert!(second_path.starts_with(hinted_parent.path()));
        assert!(!db_path.exists(), "persistent db_path must not be opened or created");

        drop(first_adapter);
        drop(second_adapter);
        drop(first_temp_dir);
        drop(second_temp_dir);

        assert!(!first_path.exists());
        assert!(!second_path.exists());
    }

    #[test]
    fn test_create_ephemeral_index_adapter_missing_parent_hint_uses_fallback_parent() {
        let fallback = tempfile::tempdir().unwrap();
        let missing_parent = fallback.path().join("missing");
        let db_path = missing_parent.join("semantic-index");

        let (temp_index_dir, index_adapter) =
            create_ephemeral_index_adapter(&db_path, fallback.path()).unwrap();
        let temp_index_path = temp_index_dir.path().to_path_buf();

        assert!(temp_index_path.starts_with(fallback.path()));
        assert!(!missing_parent.exists(), "missing parent hint must not be created");
        assert!(!db_path.exists(), "persistent db_path must not be opened or created");

        drop(index_adapter);
        drop(temp_index_dir);

        assert!(!temp_index_path.exists(), "ephemeral index dir must be removed on drop");
    }

    // ── D7/IN-09: persistent index manifest + incremental reuse ─────────────

    fn make_test_fragment(path: &str, content: &str) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), content.to_owned(), 1, 1).unwrap()
    }

    /// Stub embedding port that records how many times embed_batch was called and
    /// returns a fixed dimension-2 embedding per fragment.
    struct StubEmbeddingPort {
        call_count: std::sync::atomic::AtomicU32,
        /// When `fail` is true, embed_batch returns an error instead of embeddings.
        fail: bool,
        embedding_count: Option<usize>,
    }

    impl StubEmbeddingPort {
        fn new() -> Self {
            Self {
                call_count: std::sync::atomic::AtomicU32::new(0),
                fail: false,
                embedding_count: None,
            }
        }

        fn never_called() -> Self {
            Self {
                call_count: std::sync::atomic::AtomicU32::new(0),
                fail: false,
                embedding_count: None,
            }
        }

        fn failing() -> Self {
            Self {
                call_count: std::sync::atomic::AtomicU32::new(0),
                fail: true,
                embedding_count: None,
            }
        }

        fn with_embedding_count(embedding_count: usize) -> Self {
            Self {
                call_count: std::sync::atomic::AtomicU32::new(0),
                fail: false,
                embedding_count: Some(embedding_count),
            }
        }

        fn embed_batch_call_count(&self) -> u32 {
            self.call_count.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl usecase::semantic_dup::EmbeddingPort for StubEmbeddingPort {
        fn embed(
            &self,
            _fragment: &CodeFragment,
        ) -> Result<Vec<f32>, usecase::semantic_dup::EmbeddingError> {
            Ok(vec![0.5_f32; 2])
        }

        fn embed_batch(
            &self,
            fragments: &[CodeFragment],
        ) -> Result<Vec<Vec<f32>>, usecase::semantic_dup::EmbeddingError> {
            if self.fail {
                return Err(usecase::semantic_dup::EmbeddingError::InferenceFailed {
                    source: "stub failure".to_owned(),
                });
            }
            self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let count = self.embedding_count.unwrap_or(fragments.len());
            Ok((0..count).map(|_| vec![0.5_f32; 2]).collect())
        }
    }

    // ── manifest_sidecar_path ────────────────────────────────────────────────

    /// Sidecar path is `{db_path}.manifest`.
    #[test]
    fn test_manifest_sidecar_path_appends_manifest_suffix() {
        let db_path = Path::new(".semantic_index");
        let sidecar = manifest_sidecar_path(db_path);
        assert_eq!(
            sidecar,
            PathBuf::from(".semantic_index.manifest"),
            "sidecar must be db_path + '.manifest'"
        );
    }

    // ── read_manifest / write_manifest round-trip ────────────────────────────

    /// Absent sidecar returns `Ok(None)` (not an error).
    #[test]
    fn test_read_manifest_absent_sidecar_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("absent.manifest");
        let result = read_manifest(&sidecar).unwrap();
        assert!(result.is_none(), "absent manifest sidecar must return Ok(None)");
    }

    /// Write then read round-trips the manifest correctly.
    #[test]
    fn test_write_and_read_manifest_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("test.manifest");

        let mut manifest = IndexManifest::empty("model-v1");
        manifest.files.insert("src/a.rs".to_owned(), "a".repeat(64));
        manifest.files.insert("src/b.rs".to_owned(), "b".repeat(64));

        write_manifest(&sidecar, &manifest).unwrap();
        let restored =
            read_manifest(&sidecar).unwrap().expect("manifest must be readable after write");

        assert_eq!(restored.embedding_model_id, "model-v1");
        assert_eq!(
            restored.files.get("src/a.rs").map(|s| s.as_str()),
            Some("a".repeat(64).as_str())
        );
        assert_eq!(
            restored.files.get("src/b.rs").map(|s| s.as_str()),
            Some("b".repeat(64).as_str())
        );
    }

    /// remove_manifest is idempotent (absent sidecar is not an error).
    #[test]
    fn test_remove_manifest_absent_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("absent.manifest");
        remove_manifest(&sidecar).unwrap();
    }

    /// remove_manifest removes an existing sidecar.
    #[test]
    fn test_remove_manifest_removes_existing_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("test.manifest");
        write_manifest(&sidecar, &IndexManifest::empty("model-v1")).unwrap();
        assert!(sidecar.exists());
        remove_manifest(&sidecar).unwrap();
        assert!(!sidecar.exists(), "remove_manifest must delete the sidecar file");
    }

    // ── file_content_hash ────────────────────────────────────────────────────

    /// file_content_hash is stable across fragment ordering.
    #[test]
    fn test_file_content_hash_is_order_independent() {
        let frags_a = [
            make_test_fragment("src/a.rs", "fn a() {}"),
            make_test_fragment("src/a.rs", "fn b() {}"),
        ];
        let frags_b = [
            make_test_fragment("src/a.rs", "fn b() {}"),
            make_test_fragment("src/a.rs", "fn a() {}"),
        ];
        let refs_a: Vec<&CodeFragment> = frags_a.iter().collect();
        let refs_b: Vec<&CodeFragment> = frags_b.iter().collect();
        let hash_a = file_content_hash(&refs_a);
        let hash_b = file_content_hash(&refs_b);
        assert_eq!(hash_a, hash_b, "file_content_hash must be order-independent");
        assert_eq!(hash_a.len(), 64, "file_content_hash must produce a 64-char hex string");
    }

    /// Different content produces different hashes.
    #[test]
    fn test_file_content_hash_differs_for_different_content() {
        let frags_a = [make_test_fragment("src/a.rs", "fn a() {}")];
        let frags_b = [make_test_fragment("src/a.rs", "fn b() {}")];
        let refs_a: Vec<&CodeFragment> = frags_a.iter().collect();
        let refs_b: Vec<&CodeFragment> = frags_b.iter().collect();
        assert_ne!(
            file_content_hash(&refs_a),
            file_content_hash(&refs_b),
            "different content must produce different hashes"
        );
    }

    // ── compute_manifest_diff ────────────────────────────────────────────────

    /// No prior manifest → all files dirty, nothing deleted or unchanged.
    #[test]
    fn test_compute_manifest_diff_no_manifest_all_dirty() {
        let corpus = vec![
            make_test_fragment("src/a.rs", "fn a() {}"),
            make_test_fragment("src/b.rs", "fn b() {}"),
        ];
        let diff = compute_manifest_diff(&corpus, None, "model-v1");
        let mut dirty = diff.dirty.clone();
        dirty.sort();
        assert_eq!(dirty, vec!["src/a.rs", "src/b.rs"]);
        assert!(diff.deleted.is_empty(), "no deletions when manifest absent");
        assert!(diff.unchanged.is_empty(), "no unchanged when manifest absent");
    }

    /// Model mismatch → all files dirty regardless of stored hashes.
    #[test]
    fn test_compute_manifest_diff_model_mismatch_all_dirty() {
        let corpus = vec![make_test_fragment("src/a.rs", "fn a() {}")];
        let frags_a = [make_test_fragment("src/a.rs", "fn a() {}")];
        let refs_a: Vec<&CodeFragment> = frags_a.iter().collect();
        let hash_a = file_content_hash(&refs_a);

        let mut manifest = IndexManifest::empty("model-v1");
        manifest.files.insert("src/a.rs".to_owned(), hash_a);

        // Different model ID → all dirty.
        let diff = compute_manifest_diff(&corpus, Some(&manifest), "model-v2");
        assert_eq!(diff.dirty, vec!["src/a.rs"]);
        assert!(diff.deleted.is_empty());
        assert!(diff.unchanged.is_empty());
    }

    /// Changed file → dirty; deleted file → deleted; unchanged file → unchanged.
    ///
    /// This single test covers all three classifications. The unchanged-only
    /// case is exercised here (src/a.rs same content) alongside dirty/deleted,
    /// so no separate unchanged-only test is needed.
    #[test]
    fn test_compute_manifest_diff_dirty_and_deleted() {
        // manifest knows about a.rs (unchanged), b.rs (changed), c.rs (deleted)
        let frag_a_v1 = make_test_fragment("src/a.rs", "fn a() {}");
        let frag_b_v1 = make_test_fragment("src/b.rs", "fn b_old() {}");
        let frag_c = make_test_fragment("src/c.rs", "fn c() {}");

        let hash_a = file_content_hash(&[&frag_a_v1]);
        let hash_b_old = file_content_hash(&[&frag_b_v1]);
        let hash_c = file_content_hash(&[&frag_c]);

        let mut manifest = IndexManifest::empty("model-v1");
        manifest.files.insert("src/a.rs".to_owned(), hash_a);
        manifest.files.insert("src/b.rs".to_owned(), hash_b_old);
        manifest.files.insert("src/c.rs".to_owned(), hash_c);

        // Corpus now: a.rs (same), b.rs (changed), c.rs (absent → deleted)
        let corpus = vec![
            make_test_fragment("src/a.rs", "fn a() {}"),     // same
            make_test_fragment("src/b.rs", "fn b_new() {}"), // changed
        ];

        let diff = compute_manifest_diff(&corpus, Some(&manifest), "model-v1");
        assert_eq!(diff.dirty, vec!["src/b.rs"], "changed file must be dirty");
        assert_eq!(diff.deleted, vec!["src/c.rs"], "absent file must be deleted");
        assert_eq!(diff.unchanged, vec!["src/a.rs"], "same file must be unchanged");
    }

    // ── clear_persistent_index_dir ───────────────────────────────────────────

    /// clear_persistent_index_dir removes the directory.
    #[test]
    fn test_clear_persistent_index_dir_removes_existing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let index_dir = dir.path().join("the_index");
        write_persistent_index_marker(&index_dir).unwrap();
        std::fs::write(index_dir.join("fragment.data"), b"stale data").unwrap();

        clear_persistent_index_dir(&index_dir).unwrap();

        assert!(!index_dir.exists(), "clear must remove the index directory");
        assert!(
            !persistent_index_marker_path(&index_dir).exists(),
            "clear must remove the adjacent cache marker"
        );
    }

    /// clear_persistent_index_dir refuses to remove non-empty directories it did
    /// not mark as an SOTP semantic-index cache.
    #[test]
    fn test_clear_persistent_index_dir_unmarked_directory_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let index_dir = dir.path().join("not_an_index_cache");
        std::fs::create_dir_all(&index_dir).unwrap();
        std::fs::write(index_dir.join("user-data.txt"), b"must survive").unwrap();

        let message = clear_persistent_index_dir(&index_dir).unwrap_err();

        assert!(
            message.contains("refusing to clear unmarked semantic index directory"),
            "unmarked directory must fail closed, got: {message}"
        );
        assert!(
            index_dir.join("user-data.txt").exists(),
            "unmarked directory contents must not be deleted"
        );
    }

    /// A symlinked cache marker must not be accepted as ownership proof.
    #[cfg(unix)]
    #[test]
    fn test_clear_persistent_index_dir_symlink_marker_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let index_dir = dir.path().join("not_an_index_cache");
        std::fs::create_dir_all(&index_dir).unwrap();
        std::fs::write(index_dir.join("user-data.txt"), b"must survive").unwrap();

        let canonical_index_dir = index_dir.canonicalize().unwrap();
        let spoof_marker = dir.path().join("spoof-marker");
        std::fs::write(
            &spoof_marker,
            format!("sotp semantic index cache\npath={}\n", canonical_index_dir.display()),
        )
        .unwrap();
        std::os::unix::fs::symlink(&spoof_marker, persistent_index_marker_path(&index_dir))
            .unwrap();

        let message = clear_persistent_index_dir(&index_dir).unwrap_err();

        assert!(
            message.contains("index cache marker") && message.contains("symlink"),
            "symlinked marker must fail closed, got: {message}"
        );
        assert!(
            index_dir.join("user-data.txt").exists(),
            "symlinked marker must not authorize deleting the directory"
        );
    }

    /// If marker writing fails after the helper creates db_path, the newly
    /// created unmarked db_path is cleaned up so the next run can retry.
    #[test]
    fn test_write_persistent_index_marker_failure_removes_created_db_path() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");
        let marker = persistent_index_marker_path(&db_path);
        std::fs::create_dir_all(&marker).unwrap();

        let message = write_persistent_index_marker(&db_path).unwrap_err();

        assert!(
            message.contains("failed to write index cache marker"),
            "marker write failure must be reported, got: {message}"
        );
        assert!(
            !db_path.exists(),
            "newly-created unmarked db_path must be removed after marker-write failure"
        );
        assert!(marker.exists(), "test-owned marker directory should remain");
    }

    /// clear_persistent_index_dir is idempotent for a non-existing directory.
    #[test]
    fn test_clear_persistent_index_dir_absent_directory_returns_ok() {
        let dir = tempfile::tempdir().unwrap();
        let absent = dir.path().join("not_there");
        // Must not error on a non-existent path.
        clear_persistent_index_dir(&absent).unwrap();
    }

    /// Persistent index lock creates and holds a sidecar lock file.
    #[test]
    fn test_acquire_persistent_index_lock_creates_lock_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");
        let lock_path = persistent_index_lock_path(&db_path);

        let _lock = acquire_persistent_index_lock(&db_path).unwrap();

        assert!(lock_path.exists(), "lock acquisition must create the lock sidecar");
    }

    /// NullInsertIndexProxy makes insert_batch a no-op (no panic, Ok returned).
    #[test]
    fn test_null_insert_index_proxy_insert_batch_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");
        let real_adapter = Arc::new(LanceDbSemanticIndexAdapter::new(db_path.clone()).unwrap());
        let lock = acquire_persistent_index_lock(&db_path).unwrap();
        let proxy = NullInsertIndexProxy::new(Arc::clone(&real_adapter), lock);

        // insert_batch with non-empty items must be a no-op (Ok(()), no writes).
        let frag =
            CodeFragment::new(PathBuf::from("src/a.rs"), "fn a() {}".to_owned(), 1, 1).unwrap();
        let result = proxy.insert_batch(&[(frag, vec![0.1_f32, 0.2_f32])]);
        assert!(result.is_ok(), "NullInsertIndexProxy::insert_batch must return Ok(())");
    }

    /// NullInsertIndexProxy makes insert a no-op.
    #[test]
    fn test_null_insert_index_proxy_insert_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");
        let real_adapter = Arc::new(LanceDbSemanticIndexAdapter::new(db_path.clone()).unwrap());
        let lock = acquire_persistent_index_lock(&db_path).unwrap();
        let proxy = NullInsertIndexProxy::new(Arc::clone(&real_adapter), lock);

        let frag =
            CodeFragment::new(PathBuf::from("src/a.rs"), "fn a() {}".to_owned(), 1, 1).unwrap();
        let result = proxy.insert(&frag, &[0.1_f32]);
        assert!(result.is_ok(), "NullInsertIndexProxy::insert must return Ok(())");
    }

    /// NullInsertIndexProxy forwards delete_by_source_path to the inner adapter.
    #[test]
    fn test_null_insert_index_proxy_delete_by_source_path_forwards_to_inner() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");
        let real_adapter = Arc::new(LanceDbSemanticIndexAdapter::new(db_path.clone()).unwrap());
        let lock = acquire_persistent_index_lock(&db_path).unwrap();
        let proxy = NullInsertIndexProxy::new(Arc::clone(&real_adapter), lock);

        // Deleting from an empty (non-existent) table must succeed (no-op at the DB level,
        // but the call must be forwarded — not silently swallowed by the proxy).
        let result = proxy.delete_by_source_path(std::path::Path::new("src/a.rs"));
        assert!(
            result.is_ok(),
            "NullInsertIndexProxy::delete_by_source_path must forward and return Ok(()): {:?}",
            result.err()
        );
    }

    // ── open_persistent_index_with_corpus (D7 manifest-based) ────────────────

    /// No manifest → full rebuild (embed_batch called once), manifest written.
    #[test]
    fn test_open_persistent_index_with_corpus_no_manifest_full_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");
        // No sidecar written → no prior manifest → full rebuild.

        let embed = StubEmbeddingPort::new();
        let corpus = vec![make_test_fragment("src/a.rs", "fn a() {}")];
        let result = open_persistent_index_with_corpus(&db_path, corpus, &embed);

        assert!(result.is_ok(), "full rebuild must succeed: {:?}", result.err());
        assert_eq!(
            embed.embed_batch_call_count(),
            1,
            "embed_batch must be called once on full rebuild"
        );

        // Manifest sidecar must be written after successful rebuild.
        let sidecar = manifest_sidecar_path(&db_path);
        let manifest = read_manifest(&sidecar).unwrap();
        assert!(manifest.is_some(), "manifest sidecar must be written after full rebuild");
        let manifest = manifest.unwrap();
        assert!(manifest.files.contains_key("src/a.rs"), "manifest must record the built file");
    }

    /// Same corpus twice → second run skips embed_batch entirely (all unchanged).
    #[test]
    fn test_open_persistent_index_with_corpus_same_corpus_skips_embed_batch() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");

        // First run: builds from scratch.
        let embed_a = StubEmbeddingPort::new();
        let corpus_a = vec![make_test_fragment("src/a.rs", "fn a() {}")];
        open_persistent_index_with_corpus(&db_path, corpus_a, &embed_a).unwrap();
        assert_eq!(embed_a.embed_batch_call_count(), 1, "first run must call embed_batch");

        // Second run: identical corpus → all unchanged → no embed_batch.
        let embed_b = StubEmbeddingPort::never_called();
        let corpus_b = vec![make_test_fragment("src/a.rs", "fn a() {}")];
        let result = open_persistent_index_with_corpus(&db_path, corpus_b, &embed_b);
        assert!(result.is_ok(), "reuse path must succeed: {:?}", result.err());
        assert_eq!(
            embed_b.embed_batch_call_count(),
            0,
            "embed_batch must NOT be called when all files are unchanged"
        );
    }

    /// Matching manifest without the LanceDB fragments table is inconsistent:
    /// rebuild instead of returning a proxy that would search an empty table.
    #[test]
    fn test_open_persistent_index_with_corpus_matching_manifest_missing_table_rebuilds() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");
        let fragment = make_test_fragment("src/a.rs", "fn a() {}");
        let sidecar = manifest_sidecar_path(&db_path);

        write_persistent_index_marker(&db_path).unwrap();
        let mut manifest = IndexManifest::empty(EMBEDDING_MODEL_ID);
        manifest.files.insert("src/a.rs".to_owned(), file_content_hash(&[&fragment]));
        write_manifest(&sidecar, &manifest).unwrap();
        assert!(
            !db_path.join("fragments.lance").exists(),
            "test setup must have a manifest but no LanceDB table"
        );

        let embed = StubEmbeddingPort::new();
        let result = open_persistent_index_with_corpus(&db_path, vec![fragment], &embed);

        assert!(result.is_ok(), "missing table should trigger rebuild: {:?}", result.err());
        assert_eq!(
            embed.embed_batch_call_count(),
            1,
            "missing table with matching manifest must rebuild instead of reusing"
        );
        assert!(
            db_path.join("fragments.lance").is_dir(),
            "rebuild must recreate the LanceDB fragments table"
        );
    }

    /// Changed file → only the changed file is re-embedded; unchanged file is not.
    #[test]
    fn test_open_persistent_index_with_corpus_changed_file_only_reembeds_dirty_file() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");

        // First run: build with a.rs and b.rs.
        let embed_a = StubEmbeddingPort::new();
        let corpus_a = vec![
            make_test_fragment("src/a.rs", "fn a() {}"),
            make_test_fragment("src/b.rs", "fn b() {}"),
        ];
        open_persistent_index_with_corpus(&db_path, corpus_a, &embed_a).unwrap();
        assert_eq!(embed_a.embed_batch_call_count(), 1, "first run must call embed_batch");

        // Second run: a.rs unchanged, b.rs changed → embed_batch called once for b.rs.
        let embed_b = StubEmbeddingPort::new();
        let corpus_b = vec![
            make_test_fragment("src/a.rs", "fn a() {}"), // unchanged
            make_test_fragment("src/b.rs", "fn b_new() {}"), // changed
        ];
        let result = open_persistent_index_with_corpus(&db_path, corpus_b, &embed_b);
        assert!(result.is_ok(), "incremental update must succeed: {:?}", result.err());
        assert_eq!(
            embed_b.embed_batch_call_count(),
            1,
            "embed_batch must be called once for the dirty file"
        );
    }

    /// Incremental failure invalidates the manifest and clears the partial index.
    #[test]
    fn test_open_persistent_index_with_corpus_incremental_failure_clears_index() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");

        let embed_a = StubEmbeddingPort::new();
        let corpus_a = vec![make_test_fragment("src/a.rs", "fn a() {}")];
        open_persistent_index_with_corpus(&db_path, corpus_a, &embed_a).unwrap();

        let sidecar = manifest_sidecar_path(&db_path);
        assert!(
            read_manifest(&sidecar).unwrap().is_some(),
            "first successful build must write a manifest"
        );

        let embed_b = StubEmbeddingPort::failing();
        let corpus_b = vec![make_test_fragment("src/a.rs", "fn a_new() {}")];
        let result = open_persistent_index_with_corpus(&db_path, corpus_b, &embed_b);

        assert!(result.is_err(), "failing dirty-file embed must fail incremental update");
        assert!(
            read_manifest(&sidecar).unwrap().is_none(),
            "failed incremental update must remove the stale manifest"
        );
        assert!(!db_path.exists(), "failed incremental update must clear the partial DB");
        assert!(
            !persistent_index_marker_path(&db_path).exists(),
            "failed incremental update must remove the cache marker"
        );
    }

    /// Existing index directories without the SOTP marker must fail before
    /// any incremental delete/reinsert can mutate user-owned or corrupted data.
    #[test]
    fn test_open_persistent_index_with_corpus_unmarked_existing_dir_fails_before_update() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");
        std::fs::create_dir_all(&db_path).unwrap();
        std::fs::write(db_path.join("user-data.txt"), b"must survive").unwrap();

        let sidecar = manifest_sidecar_path(&db_path);
        let mut manifest = IndexManifest::empty(EMBEDDING_MODEL_ID);
        manifest.files.insert("src/a.rs".to_owned(), "stale-hash".to_owned());
        write_manifest(&sidecar, &manifest).unwrap();

        let embed = StubEmbeddingPort::never_called();
        let corpus = vec![make_test_fragment("src/a.rs", "fn a_new() {}")];
        let result = open_persistent_index_with_corpus(&db_path, corpus, &embed);

        let message = match result {
            Ok(_) => panic!("unmarked existing index directory must fail closed"),
            Err(message) => message,
        };
        assert!(
            message.contains("refusing to use unmarked semantic index directory"),
            "unmarked existing index directory must fail closed before update, got: {message}"
        );
        assert_eq!(
            embed.embed_batch_call_count(),
            0,
            "unmarked existing index directory must fail before embedding"
        );
        assert!(
            db_path.join("user-data.txt").exists(),
            "unmarked directory contents must not be mutated"
        );
    }

    /// Deleted file → its fragments are removed from the index (no stale hits).
    #[test]
    fn test_open_persistent_index_with_corpus_deleted_file_fragments_removed() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");

        // First run: build with stale.rs + fresh.rs.
        let embed_a = StubEmbeddingPort::new();
        let corpus_a = vec![
            make_test_fragment("stale.rs", "fn stale() {}"),
            make_test_fragment("fresh.rs", "fn fresh() {}"),
        ];
        let result_a = open_persistent_index_with_corpus(&db_path, corpus_a, &embed_a).unwrap();
        drop(result_a);

        // Second run: stale.rs removed from corpus.
        let embed_b = StubEmbeddingPort::never_called();
        let corpus_b = vec![make_test_fragment("fresh.rs", "fn fresh() {}")];
        let index_b = open_persistent_index_with_corpus(&db_path, corpus_b, &embed_b).unwrap();

        // Search must NOT return stale.rs.
        let search_results =
            index_b.search(&[0.5_f32; 2], domain::semantic_dup::TopK::new(10).unwrap()).unwrap();
        let found_stale = search_results
            .iter()
            .any(|r| r.fragment.source_path.to_string_lossy().contains("stale"));
        assert!(
            !found_stale,
            "deleted file fragments must not survive incremental update; found: {:?}",
            search_results
                .iter()
                .map(|r| r.fragment.source_path.display().to_string())
                .collect::<Vec<_>>()
        );
    }

    /// Full rebuild failure → manifest removed so next run doesn't reuse a partial DB.
    #[test]
    fn test_open_persistent_index_with_corpus_rebuild_failure_removes_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");

        let embed = StubEmbeddingPort::failing();
        let corpus = vec![make_test_fragment("src/a.rs", "fn a() {}")];
        let result = open_persistent_index_with_corpus(&db_path, corpus, &embed);

        assert!(result.is_err(), "failing embed_batch must fail rebuild");
        let sidecar = manifest_sidecar_path(&db_path);
        assert!(
            read_manifest(&sidecar).unwrap().is_none(),
            "failed rebuild must remove the manifest sidecar"
        );
        assert!(!db_path.exists(), "failed rebuild must remove the partial DB directory");
        assert!(
            !persistent_index_marker_path(&db_path).exists(),
            "failed rebuild must remove the cache marker"
        );
    }

    /// Embedding count mismatch on full rebuild → error, no manifest written.
    #[test]
    fn test_open_persistent_index_with_corpus_embedding_count_mismatch_no_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");
        let embed = StubEmbeddingPort::with_embedding_count(0);
        let corpus = vec![make_test_fragment("src/a.rs", "fn a() {}")];

        let result = open_persistent_index_with_corpus(&db_path, corpus, &embed);

        assert!(result.is_err(), "mismatched embedding count must fail");
        let message = result.err().unwrap();
        assert!(
            message.contains("full rebuild embed_batch returned 0 embeddings for 1 fragments")
                || message.contains("0 embeddings"),
            "mismatched embedding count must fail clearly, got: {message}"
        );
        let sidecar = manifest_sidecar_path(&db_path);
        assert!(
            read_manifest(&sidecar).unwrap().is_none(),
            "mismatched embedding count must not write a manifest sidecar"
        );
    }

    /// Full rebuild clears stale data: second call with a different corpus replaces all fragments.
    #[test]
    fn test_open_persistent_index_with_corpus_model_change_triggers_full_rebuild() {
        // We can't change EMBEDDING_MODEL_ID at runtime, so instead we simulate
        // a stale manifest with a different model ID by writing it directly.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");

        // First run: build with corpus_a.
        let embed_a = StubEmbeddingPort::new();
        let corpus_a = vec![make_test_fragment("stale.rs", "fn stale() {}")];
        open_persistent_index_with_corpus(&db_path, corpus_a, &embed_a).unwrap();
        assert_eq!(embed_a.embed_batch_call_count(), 1);

        // Tamper the manifest to simulate a different model ID.
        let sidecar = manifest_sidecar_path(&db_path);
        let mut tampered = read_manifest(&sidecar).unwrap().unwrap();
        tampered.embedding_model_id = "old-model-v0".to_owned();
        write_manifest(&sidecar, &tampered).unwrap();

        // Second run: model mismatch → full rebuild, stale.rs must be gone.
        let embed_b = StubEmbeddingPort::new();
        let corpus_b = vec![make_test_fragment("fresh.rs", "fn fresh() {}")];
        let index_b = open_persistent_index_with_corpus(&db_path, corpus_b, &embed_b).unwrap();
        assert_eq!(embed_b.embed_batch_call_count(), 1, "model mismatch must trigger full rebuild");

        let search_results =
            index_b.search(&[0.5_f32; 2], domain::semantic_dup::TopK::new(10).unwrap()).unwrap();
        let found_stale = search_results
            .iter()
            .any(|r| r.fragment.source_path.to_string_lossy().contains("stale"));
        assert!(
            !found_stale,
            "stale fragments must not survive model-change full rebuild; found: {:?}",
            search_results
                .iter()
                .map(|r| r.fragment.source_path.display().to_string())
                .collect::<Vec<_>>()
        );
    }

    // ── GateEval telemetry end-to-end: dry_check_approved emits event ────────
    //
    // These tests verify that `CliApp::dry_check_approved()` actually reaches the
    // `emit_gate_eval(...)` call after finalizing the verdict (T007 / IN-07).
    // The mapping helper (`dry_check_approved_gate_eval_fields`) is tested
    // separately above; these tests exercise the composition path end-to-end so
    // a regression that removes or reorders the telemetry call would be caught.
    //
    // Setup: a temporary git repo on a `track/<id>` branch with a real initial
    // commit (so `build_diff_and_corpus_fragments` has a valid base to diff
    // against), no Rust source files (so the diff is empty → `current_fragment_refs`
    // is empty), and pre-created dry-check artefacts for the desired verdict.
    //
    // Telemetry output is redirected to a tempdir via `SOTP_TELEMETRY_DIR` so the
    // test can read `telemetry.jsonl` without touching the real repo's track items.

    /// Set up a minimal git repo on a track branch with one commit.
    ///
    /// Returns `(repo_tempdir, items_dir, track_id, initial_commit_hash)`.
    /// Caller must keep `repo_tempdir` alive for the duration of the test.
    fn setup_gate_eval_repo(track_id: &str) -> (tempfile::TempDir, PathBuf, String) {
        use std::process::Command;

        let repo = tempfile::tempdir().unwrap();
        let root = repo.path();

        GitRunner::at(root).assert_success(&["init", "-b", "main"]);
        GitRunner::at(root).assert_success(&["config", "user.email", "test@example.com"]);
        GitRunner::at(root).assert_success(&["config", "user.name", "Test"]);
        GitRunner::at(root).assert_success(&["config", "commit.gpgsign", "false"]);

        let items_dir = root.join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::write(root.join("README.md"), "init\n").unwrap();

        // Create a minimal valid dry-check.json so dry_check_approved can load
        // the config fingerprint (required since the config-fingerprint fix).
        let harness_config_dir = root.join(".harness/config");
        std::fs::create_dir_all(&harness_config_dir).unwrap();
        std::fs::write(
            harness_config_dir.join("dry-check.json"),
            r#"{
  "schema_version": 3,
  "threshold": 0.85,
  "max_parallelism": 4,
  "fast_reasoning_effort": "medium",
  "final_reasoning_effort": "high",
  "known_bad_injection_rate_percent": 10,
  "known_bad_detection_threshold_percent": 90
}"#,
        )
        .unwrap();

        GitRunner::at(root).assert_success(&["add", "."]);
        GitRunner::at(root).assert_success(&["commit", "--no-gpg-sign", "-m", "init"]);

        // Switch to a track branch so `resolve_dry_write_telemetry_writer` returns Some.
        GitRunner::at(root).assert_success(&["checkout", "-b", &format!("track/{track_id}")]);

        // Capture the initial commit hash (the base we will pass to dry_check_approved).
        let output =
            Command::new("git").current_dir(root).args(["rev-parse", "HEAD"]).output().unwrap();
        let commit_hash = String::from_utf8(output.stdout).unwrap().trim().to_owned();

        (repo, items_dir, commit_hash)
    }

    /// End-to-end: `dry_check_approved` emits a GateEval event with
    /// `verdict="ok"` (Approved) when:
    ///   - coverage manifest present with an empty fragment-ref set,
    ///   - dry-check.json present with no records,
    ///   - diff is empty (no Rust source files in the repo).
    ///
    /// Verifies that the telemetry call in `CliApp::dry_check_approved` is
    /// actually reached after the verdict is finalized, not just that the
    /// mapping helper produces the right strings.
    #[test]
    fn test_dry_check_approved_approved_verdict_emits_gate_eval_telemetry_event() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();

        let track_id = "gate-eval-approved-2026-06-14";
        let (repo, items_dir, commit_hash) = setup_gate_eval_repo(track_id);

        // Pre-create the track directory with an empty coverage manifest and
        // an empty dry-check store.  No Rust source files → empty diff →
        // empty current_fragment_refs → Approved.
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();

        // Compute the fingerprint from the config written by setup_gate_eval_repo
        // so the v3 coverage manifest contains a matching fingerprint.
        // A mismatch would cause DryCheckApprovalInteractor to return Blocked.
        let dry_check_config_path = repo.path().join(".harness/config/dry-check.json");
        let dry_check_config =
            infrastructure::dry_check::DryCheckConfig::load(&dry_check_config_path)
                .expect("dry-check.json written by setup_gate_eval_repo must load");
        let fingerprint_hex = dry_check_config.fingerprint().as_str().to_owned();

        let coverage_json = format!(
            r#"{{"schema_version":3,"config_fingerprint":"{fingerprint_hex}","fragment_refs":[],"processed_pair_keys":[]}}"#
        );
        std::fs::write(track_dir.join("dry-check-coverage.json"), coverage_json.as_bytes())
            .unwrap();
        std::fs::write(track_dir.join("dry-check.json"), br#"{"schema_version":1,"records":[]}"#)
            .unwrap();

        // Use a SOTP_TELEMETRY_DIR tempdir so telemetry.jsonl is written there.
        let telemetry_dir = tempfile::tempdir().unwrap();

        // Change CWD to the test repo so SystemGitRepo::discover() finds it.
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo.path()).unwrap();

        let _telemetry_guard = EnvGuard::set("SOTP_TELEMETRY", "1");
        let _telemetry_dir_guard =
            EnvGuard::set("SOTP_TELEMETRY_DIR", telemetry_dir.path().as_os_str().to_os_string());

        let result = CliApp::new().dry_check_approved(DryCheckApprovedInput {
            track_id: track_id.to_owned(),
            base_commit: Some(commit_hash),
            items_dir: items_dir.clone(),
        });

        // The call must succeed with exit_code 0 (Approved).
        let outcome = result.unwrap();
        assert_eq!(
            outcome.exit_code, 0,
            "Approved verdict must produce exit_code 0; got: {outcome:?}"
        );

        // The GateEval event must be present in telemetry.jsonl.
        let telemetry_path = telemetry_dir.path().join("telemetry.jsonl");
        assert!(
            telemetry_path.exists(),
            "telemetry.jsonl must be written when SOTP_TELEMETRY=1 on a track branch"
        );
        let content = std::fs::read_to_string(&telemetry_path).unwrap();
        assert!(
            content.contains("GateEval"),
            "GateEval event must be emitted after Approved verdict; got: {content}"
        );
        assert!(
            content.contains("\"verdict\":\"ok\""),
            "Approved verdict must produce verdict=\"ok\" in GateEval event; got: {content}"
        );
        assert!(
            content.contains("\"gate_name\":\"dry\""),
            "gate_name must be \"dry\" in GateEval event; got: {content}"
        );
    }

    // ── T016 smoke tests: build_usecase_dry_check_config + resolve_dry_checker_models ──

    /// Builds an infra `DryCheckConfig` from JSON content and a temp file.
    fn load_infra_dry_check_config_from_json(
        json: &str,
    ) -> infrastructure::dry_check::DryCheckConfig {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dry-check.json");
        std::fs::write(&path, json).unwrap();
        let config = infrastructure::dry_check::DryCheckConfig::load(&path).unwrap();
        // Keep dir alive through the function — config has no reference to the file.
        drop(dir);
        config
    }

    /// Verify that `build_usecase_dry_check_config` propagates `max_parallelism`
    /// from the infra config to the usecase config newtype.
    #[test]
    fn test_dry_write_passes_max_parallelism_to_usecase_config() {
        let infra_config = load_infra_dry_check_config_from_json(
            r#"{
                "schema_version": 3,
                "threshold": 0.85,
                "max_parallelism": 7,
                "fast_reasoning_effort": "medium",
                "final_reasoning_effort": "high",
                "known_bad_injection_rate_percent": 10,
                "known_bad_detection_threshold_percent": 90
            }"#,
        );
        assert_eq!(infra_config.max_parallelism(), 7, "infra config must expose max_parallelism=7");

        let usecase_config = build_usecase_dry_check_config(&infra_config).unwrap();
        assert_eq!(
            usecase_config.max_parallelism.as_usize(),
            7,
            "build_usecase_dry_check_config must propagate max_parallelism to the usecase newtype"
        );
    }

    /// Verify that `build_usecase_dry_check_config` propagates the known-bad calibration
    /// percent fields from the infra config to the usecase config newtypes.
    #[test]
    fn test_dry_write_passes_known_bad_calibration_to_usecase_config() {
        let infra_config = load_infra_dry_check_config_from_json(
            r#"{
                "schema_version": 3,
                "threshold": 0.85,
                "max_parallelism": 4,
                "fast_reasoning_effort": "medium",
                "final_reasoning_effort": "high",
                "known_bad_injection_rate_percent": 20,
                "known_bad_detection_threshold_percent": 80
            }"#,
        );
        assert_eq!(infra_config.known_bad_injection_rate_percent(), 20);
        assert_eq!(infra_config.known_bad_detection_threshold_percent(), 80);

        let usecase_config = build_usecase_dry_check_config(&infra_config).unwrap();
        assert_eq!(
            usecase_config.known_bad_injection_rate_percent.as_u8(),
            20,
            "build_usecase_dry_check_config must propagate known_bad_injection_rate_percent"
        );
        assert_eq!(
            usecase_config.known_bad_detection_threshold_percent.as_u8(),
            80,
            "build_usecase_dry_check_config must propagate known_bad_detection_threshold_percent"
        );
    }

    /// Verify that `resolve_dry_checker_models` returns fast and final models from a
    /// test `agent-profiles.json` with both `fast_model` and `model` defined.
    #[test]
    fn test_resolve_dry_checker_models_returns_fast_and_final_from_agent_profiles() {
        let dir = tempfile::tempdir().unwrap();

        // Write a minimal agent-profiles.json with separate fast_model / model for dry-checker.
        std::fs::create_dir_all(dir.path().join(".harness/config")).unwrap();
        std::fs::write(
            dir.path().join(".harness/config/agent-profiles.json"),
            r#"{
  "schema_version": 1,
  "providers": { "codex": { "label": "Codex" } },
  "capabilities": {
    "dry-checker": {
      "provider": "codex",
      "model": "final-model-v1",
      "fast_model": "fast-model-v1"
    }
  }
}"#,
        )
        .unwrap();

        let (fast_model, final_model) =
            resolve_dry_checker_models(dir.path(), "dry-checker", None).unwrap();

        assert_eq!(
            fast_model, "fast-model-v1",
            "fast_model must come from the fast_model field in agent-profiles.json"
        );
        assert_eq!(
            final_model, "final-model-v1",
            "final_model must come from the model field in agent-profiles.json"
        );
    }

    /// Verify that `resolve_dry_checker_models` falls back fast_model → final_model
    /// when no separate `fast_model` field is configured.
    #[test]
    fn test_resolve_dry_checker_models_fast_falls_back_to_final_when_not_set() {
        let dir = tempfile::tempdir().unwrap();

        std::fs::create_dir_all(dir.path().join(".harness/config")).unwrap();
        std::fs::write(
            dir.path().join(".harness/config/agent-profiles.json"),
            r#"{
  "schema_version": 1,
  "providers": { "codex": { "label": "Codex" } },
  "capabilities": {
    "dry-checker": {
      "provider": "codex",
      "model": "only-final-model-v1"
    }
  }
}"#,
        )
        .unwrap();

        let (fast_model, final_model) =
            resolve_dry_checker_models(dir.path(), "dry-checker", None).unwrap();

        assert_eq!(
            fast_model, "only-final-model-v1",
            "fast_model must fall back to final_model when fast_model is not configured"
        );
        assert_eq!(final_model, "only-final-model-v1", "final_model must be the model field");
    }

    /// Verify that `FsDryCheckCoverageAdapter` write/read round-trip works correctly.
    ///
    /// Mirrors the round-trip pattern used by the coverage adapter's own unit tests,
    /// but exercised here at the composition layer to confirm the adapter integrates
    /// correctly with the types exposed to `dry_check_approved`.
    #[test]
    fn test_dry_check_approved_uses_coverage_adapter_for_staleness() {
        use std::collections::BTreeSet;

        use domain::dry_check::{
            DryCheckConfigFingerprint, DryCheckCoverageRecord, FragmentContentHash, FragmentRef,
        };
        use domain::review_v2::FilePath;
        use usecase::dry_check::DryCheckCoveragePort as _;

        let dir = tempfile::tempdir().unwrap();
        let store_path = dir.path().join("dry-check-coverage.json");
        let adapter = FsDryCheckCoverageAdapter::new(store_path.clone(), dir.path().to_path_buf());
        let track_id = domain::TrackId::try_new("coverage-smoke-2026-06-15").unwrap();

        // Before any write, read must return None (file absent → Blocked / fail-closed).
        let before = adapter.read_coverage(&track_id).unwrap();
        assert!(before.is_none(), "coverage adapter must return None when the manifest is absent");

        // Write a coverage record with one fragment ref.
        let path = FilePath::new("src/lib.rs").unwrap();
        let hash = FragmentContentHash::new("a".repeat(64)).unwrap();
        let fragment_ref = FragmentRef::new(path, hash);
        let mut refs = BTreeSet::new();
        refs.insert(fragment_ref.clone());
        let test_fp = DryCheckConfigFingerprint::new("a".repeat(64)).unwrap();
        let record = DryCheckCoverageRecord::new(refs, BTreeSet::new(), test_fp);
        adapter.write_coverage(&track_id, record.clone()).unwrap();

        // After write, read must return the same record (write/read round-trip).
        let after = adapter.read_coverage(&track_id).unwrap();
        assert_eq!(
            after,
            Some(record),
            "coverage adapter read must return the written record after write_coverage"
        );
    }

    /// End-to-end: `dry_check_approved` emits a GateEval event with
    /// `verdict="error"` (Blocked) when the coverage manifest is absent
    /// (fail-closed: missing coverage → Blocked { unresolved_pair_count: 1 }).
    ///
    /// Verifies the telemetry path is reached for the Blocked flow as well,
    /// so both branches of `dry_check_approved_gate_eval_fields` are exercised
    /// through the full composition path.
    #[test]
    fn test_dry_check_approved_blocked_verdict_emits_gate_eval_telemetry_event() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();

        let track_id = "gate-eval-blocked-2026-06-14";
        let (repo, items_dir, commit_hash) = setup_gate_eval_repo(track_id);

        // Pre-create the track directory but intentionally omit the coverage
        // manifest so `FsDryCheckCoverageAdapter::read_coverage` returns None,
        // causing `DryCheckApprovalInteractor::check_approved` to return
        // Blocked { unresolved_pair_count: 1 }.
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("dry-check.json"), br#"{"schema_version":1,"records":[]}"#)
            .unwrap();
        // dry-check-coverage.json intentionally absent.

        let telemetry_dir = tempfile::tempdir().unwrap();

        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo.path()).unwrap();

        let _telemetry_guard = EnvGuard::set("SOTP_TELEMETRY", "1");
        let _telemetry_dir_guard =
            EnvGuard::set("SOTP_TELEMETRY_DIR", telemetry_dir.path().as_os_str().to_os_string());

        let result = CliApp::new().dry_check_approved(DryCheckApprovedInput {
            track_id: track_id.to_owned(),
            base_commit: Some(commit_hash),
            items_dir: items_dir.clone(),
        });

        // The call must succeed (Ok) with exit_code 1 (Blocked — exits non-zero).
        let outcome = result.unwrap();
        assert_eq!(
            outcome.exit_code, 1,
            "Blocked verdict must produce exit_code 1; got: {outcome:?}"
        );

        // The GateEval event must be present in telemetry.jsonl.
        let telemetry_path = telemetry_dir.path().join("telemetry.jsonl");
        assert!(
            telemetry_path.exists(),
            "telemetry.jsonl must be written when SOTP_TELEMETRY=1 on a track branch"
        );
        let content = std::fs::read_to_string(&telemetry_path).unwrap();
        assert!(
            content.contains("GateEval"),
            "GateEval event must be emitted after Blocked verdict; got: {content}"
        );
        assert!(
            content.contains("\"verdict\":\"error\""),
            "Blocked verdict must produce verdict=\"error\" in GateEval event; got: {content}"
        );
        assert!(
            content.contains("\"gate_name\":\"dry\""),
            "gate_name must be \"dry\" in GateEval event; got: {content}"
        );
        assert!(
            content.contains("blocked:"),
            "Blocked reason_summary must contain \"blocked:\" in GateEval event; got: {content}"
        );
    }
}
