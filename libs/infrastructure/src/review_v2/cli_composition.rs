//! CLI composition root for v2 review system.
//!
//! Provides string-accepting builder functions and composition types so that
//! `apps/cli/src/commands/review/compose_v2.rs` does not need to import
//! `domain::` types directly (CN-01 / AC-03).
//!
//! All domain type conversions happen here, inside the infrastructure layer,
//! which is permitted to depend on domain.

use std::path::Path;

use domain::review_v2::{CommitHashReader, FilePath, ReviewExistsPort, ReviewScopeConfig};
use domain::{CommitHash, TrackId};

use crate::git_cli::{GitRepository, SystemGitRepo};
use crate::review_v2::{
    ClaudeReviewer, CodexReviewer, FsCommitHashStore, FsReviewStore, GitDiffGetter, NullReviewer,
    SystemReviewHasher, load_v2_scope_config,
};
use usecase::review_v2::{
    ReviewApprovalDecision, ReviewApprovalOutput, ReviewCheckApprovedError,
    ReviewCheckApprovedInteractor, ReviewCheckApprovedService, ReviewCycle, RunReviewCommand,
    RunReviewError, RunReviewInteractor, RunReviewOutput, RunReviewService, ScopeQueryInteractor,
    error::DiffGetError, ports::DiffGetter,
};

/// Stub `DiffGetter` for pure-logic use cases (e.g. `classify`) that do not
/// need diff listings. `list_diff_files` always returns an empty list.
pub struct NullDiffGetter;

impl DiffGetter for NullDiffGetter {
    fn list_diff_files(&self, _base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError> {
        Ok(Vec::new())
    }
}

/// All v2 adapters needed for status/check-approved operations (NullReviewer).
///
/// Fields are intentionally private — callers access behaviour through the
/// string-accepting free functions in this module (`check_approved_str`,
/// `render_review_results_str`, etc.) so that concrete infrastructure types
/// (`ReviewCycle`, `FsReviewStore`, …) are not exposed to the CLI layer.
///
/// `commit_hash_store` is retained for future commands that need to read or
/// reset the diff base without going through the full composition.
#[allow(dead_code)]
pub struct ReviewV2Composition {
    cycle: ReviewCycle<NullReviewer, SystemReviewHasher, GitDiffGetter>,
    review_store: FsReviewStore,
    commit_hash_store: FsCommitHashStore,
    base: CommitHash,
}

/// All v2 adapters needed for actual review (CodexReviewer).
///
/// Fields are intentionally private — callers access behaviour through the
/// string-accepting free functions in this module (`run_codex_review_str`,
/// etc.) so that concrete infrastructure types are not exposed to the CLI layer.
///
/// `commit_hash_store` and `base` are retained for future commands that need
/// to read or reset the diff base without going through the full composition.
#[allow(dead_code)]
pub struct ReviewV2CompositionWithCodex {
    cycle: ReviewCycle<CodexReviewer, SystemReviewHasher, GitDiffGetter>,
    review_store: FsReviewStore,
    commit_hash_store: FsCommitHashStore,
    base: CommitHash,
}

/// All v2 adapters needed for actual review (ClaudeReviewer).
///
/// Fields are intentionally private — callers access behaviour through the
/// string-accepting free functions in this module (`run_claude_review_str`,
/// etc.) so that concrete infrastructure types are not exposed to the CLI layer.
///
/// `commit_hash_store` and `base` are retained for future commands that need
/// to read or reset the diff base without going through the full composition.
#[allow(dead_code)]
pub struct ReviewV2CompositionWithClaude {
    cycle: ReviewCycle<ClaudeReviewer, SystemReviewHasher, GitDiffGetter>,
    review_store: FsReviewStore,
    commit_hash_store: FsCommitHashStore,
    base: CommitHash,
}

/// Builds the v2 review composition with a real `CodexReviewer`.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn build_review_v2_with_reviewer(
    track_id: &TrackId,
    items_dir: &Path,
    reviewer: CodexReviewer,
) -> Result<ReviewV2CompositionWithCodex, String> {
    let (scope_config, review_store, commit_hash_store, base) =
        build_v2_shared(track_id, items_dir)?;
    let cycle =
        ReviewCycle::new(base.clone(), scope_config, reviewer, GitDiffGetter, SystemReviewHasher);
    Ok(ReviewV2CompositionWithCodex { cycle, review_store, commit_hash_store, base })
}

/// Builds the v2 review composition.
///
/// 1. Discovers git root
/// 2. Validates that `items_dir` resolves under the repo root (path traversal guard)
/// 3. Loads review-scope.json → `ReviewScopeConfig`
/// 4. Reads `.commit_hash` → `CommitHash` (fallback: `git rev-parse main`)
/// 5. Constructs `FsReviewStore`, `FsCommitHashStore`
/// 6. Returns `ReviewCycle` with `NullReviewer` (status/check-approved only)
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn build_review_v2(
    track_id: &TrackId,
    items_dir: &Path,
) -> Result<ReviewV2Composition, String> {
    let (scope_config, review_store, commit_hash_store, base) =
        build_v2_shared(track_id, items_dir)?;
    let cycle = ReviewCycle::new(
        base.clone(),
        scope_config,
        NullReviewer,
        GitDiffGetter,
        SystemReviewHasher,
    );
    Ok(ReviewV2Composition { cycle, review_store, commit_hash_store, base })
}

/// String-accepting variant of `build_review_v2`.
///
/// Converts `track_id_str` to `TrackId` and delegates. Callers that must not
/// import `domain::TrackId` use this entry point (CN-01 / AC-03).
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn build_review_v2_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ReviewV2Composition, String> {
    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    build_review_v2(&track_id, items_dir)
}

/// String-accepting variant of `build_review_v2_with_reviewer`.
///
/// Converts `track_id_str` to `TrackId` and delegates. Callers that must not
/// import `domain::TrackId` use this entry point (CN-01 / AC-03).
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn build_review_v2_with_reviewer_str(
    track_id_str: &str,
    items_dir: &Path,
    reviewer: CodexReviewer,
) -> Result<ReviewV2CompositionWithCodex, String> {
    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    build_review_v2_with_reviewer(&track_id, items_dir, reviewer)
}

/// Builds the v2 review composition with a real `ClaudeReviewer`.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn build_review_v2_with_claude_reviewer(
    track_id: &TrackId,
    items_dir: &Path,
    reviewer: ClaudeReviewer,
) -> Result<ReviewV2CompositionWithClaude, String> {
    let (scope_config, review_store, commit_hash_store, base) =
        build_v2_shared(track_id, items_dir)?;
    let cycle =
        ReviewCycle::new(base.clone(), scope_config, reviewer, GitDiffGetter, SystemReviewHasher);
    Ok(ReviewV2CompositionWithClaude { cycle, review_store, commit_hash_store, base })
}

/// String-accepting variant of `build_review_v2_with_claude_reviewer`.
///
/// Converts `track_id_str` to `TrackId` and delegates. Callers that must not
/// import `domain::TrackId` use this entry point (CN-01 / AC-03).
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn build_review_v2_with_claude_reviewer_str(
    track_id_str: &str,
    items_dir: &Path,
    reviewer: ClaudeReviewer,
) -> Result<ReviewV2CompositionWithClaude, String> {
    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    build_review_v2_with_claude_reviewer(&track_id, items_dir, reviewer)
}

/// Resolves the diff base + diff getter for `sotp review files`.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn resolve_diff_base_and_getter(
    track_id: &TrackId,
    items_dir: &Path,
) -> Result<(GitDiffGetter, CommitHash), String> {
    let (_scope_config, _review_store, _commit_hash_store, base) =
        build_v2_shared(track_id, items_dir)?;
    Ok((GitDiffGetter, base))
}

/// Builds a `ScopeQueryInteractor` from string `track_id` and `items_dir`.
///
/// Callers that must not import `domain::TrackId` or `domain::CommitHash` use
/// this entry point (CN-01 / AC-03).
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn build_scope_query_interactor_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ScopeQueryInteractor<GitDiffGetter>, String> {
    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    let (diff_getter, base) = resolve_diff_base_and_getter(&track_id, items_dir)?;
    let scope_config = load_scope_config_only(&track_id, items_dir)?;
    Ok(ScopeQueryInteractor::new(scope_config, diff_getter, base))
}

/// Builds a `ScopeQueryInteractor` for pure-logic use (no diff I/O).
///
/// Uses [`NullDiffGetter`] and a placeholder `CommitHash`. Suitable for the
/// `sotp review classify` command which only calls
/// [`usecase::review_v2::ScopeQueryService::classify_by_strings`].
///
/// Skips the `.commit_hash` lookup and `git rev-parse main` fallback that the
/// full builder performs, so it works in repositories without a recorded diff
/// base.
///
/// # Errors
/// Returns a human-readable error string on failure (invalid track id,
/// scope-config load failure, items_dir traversal guard).
pub fn build_scope_query_interactor_no_diff_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ScopeQueryInteractor<NullDiffGetter>, String> {
    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    let scope_config = load_scope_config_only(&track_id, items_dir)?;
    let placeholder_base = CommitHash::try_new("0".repeat(40))
        .map_err(|e| format!("internal: placeholder commit hash construction failed: {e}"))?;
    Ok(ScopeQueryInteractor::new(scope_config, NullDiffGetter, placeholder_base))
}

/// Validates that `scope_name` is a configured scope for the given track,
/// without resolving the diff base.
///
/// Used by `sotp review files` to enforce AC-08 ordering: scope name validation
/// runs before any diff I/O. Returns `Ok(())` if the scope name is valid and
/// known, `Err(message)` otherwise.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn validate_scope_for_track_str(
    track_id_str: &str,
    items_dir: &Path,
    scope_name: &str,
) -> Result<(), String> {
    use domain::review_v2::{MainScopeName, ScopeName};

    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    let scope_config = load_scope_config_only(&track_id, items_dir)?;

    let scope = if scope_name.eq_ignore_ascii_case("other") {
        ScopeName::Other
    } else {
        ScopeName::Main(
            MainScopeName::new(scope_name.to_owned())
                .map_err(|e| format!("invalid scope name '{scope_name}': {e}"))?,
        )
    };

    if scope_config.contains_scope(&scope) {
        Ok(())
    } else {
        let known: Vec<String> =
            scope_config.all_scope_names().iter().map(|n| n.to_string()).collect();
        Err(format!("Unknown scope: {scope_name}. Known scopes: {}", known.join(", ")))
    }
}

/// Loads just the `ReviewScopeConfig` for a given track/items_dir, without
/// initialising review/hash stores or resolving the diff base.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn load_scope_config_only(
    track_id: &TrackId,
    items_dir: &Path,
) -> Result<ReviewScopeConfig, String> {
    let git = SystemGitRepo::discover().map_err(|e| format!("git discover: {e}"))?;
    let root = git.root().to_path_buf();

    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("failed to canonicalize repo root {}: {e}", root.display()))?;
    let items_dir_abs =
        if items_dir.is_absolute() { items_dir.to_path_buf() } else { root.join(items_dir) };
    // Canonicalize directly: resolve all symlinks and `..` components together.
    // `normalize_path_components` + partial-walk is unsound when intermediate path
    // components are symlinks (the `..` is stripped before the symlink is resolved,
    // so traversal can escape the repo root without detection).
    // If `items_dir` does not exist on the filesystem, we cannot verify containment
    // and treat it as if it were outside the repository root (fail-closed).
    let canonical_items_dir = items_dir_abs.canonicalize().map_err(|_| {
        format!(
            "items_dir '{}' is outside the repository root '{}' or does not exist. \
             Only paths under the repo are allowed.",
            items_dir.display(),
            canonical_root.display()
        )
    })?;
    if !canonical_items_dir.starts_with(&canonical_root) {
        return Err(format!(
            "items_dir '{}' is outside the repository root '{}'. \
             Only paths under the repo are allowed.",
            items_dir.display(),
            canonical_root.display()
        ));
    }

    let scope_json_path = root.join("track/review-scope.json");
    load_v2_scope_config(&scope_json_path, track_id, &root)
        .map_err(|e| format!("load review-scope.json: {e}"))
}

/// String-accepting variant of `load_scope_config_only`.
///
/// Converts `track_id_str` to `TrackId` and delegates. Returns the scope config
/// for use in the CLI composition root (via `append_scope_briefing_reference`).
/// Returns `Err` if the track ID is invalid or the config cannot be loaded.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn load_scope_config_only_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ReviewScopeConfig, String> {
    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    load_scope_config_only(&track_id, items_dir)
}

/// Shared setup logic for both `build_review_v2` and `build_review_v2_with_reviewer`.
///
/// Returns `(scope_config, review_store, commit_hash_store, base)`.
///
/// # Errors
/// Returns a human-readable error string on failure.
fn build_v2_shared(
    track_id: &TrackId,
    items_dir: &Path,
) -> Result<(ReviewScopeConfig, FsReviewStore, FsCommitHashStore, CommitHash), String> {
    let git = SystemGitRepo::discover().map_err(|e| format!("git discover: {e}"))?;
    let root = git.root().to_path_buf();

    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("failed to canonicalize repo root {}: {e}", root.display()))?;
    let items_dir_abs =
        if items_dir.is_absolute() { items_dir.to_path_buf() } else { root.join(items_dir) };
    // Canonicalize directly: resolve all symlinks and `..` components together.
    // `normalize_path_components` + partial-walk is unsound when intermediate path
    // components are symlinks (the `..` is stripped before the symlink is resolved,
    // so traversal can escape the repo root without detection).
    // If `items_dir` does not exist on the filesystem, we cannot verify containment
    // and treat it as if it were outside the repository root (fail-closed).
    let canonical_items_dir = items_dir_abs.canonicalize().map_err(|_| {
        format!(
            "items_dir '{}' is outside the repository root '{}' or does not exist. \
             Only paths under the repo are allowed.",
            items_dir.display(),
            canonical_root.display()
        )
    })?;
    if !canonical_items_dir.starts_with(&canonical_root) {
        return Err(format!(
            "items_dir '{}' is outside the repository root '{}'. \
             Only paths under the repo are allowed.",
            items_dir.display(),
            canonical_root.display()
        ));
    }

    let track_dir = items_dir_abs.join(track_id.as_ref());
    if !track_dir.is_dir() {
        return Err(format!(
            "track directory '{}' does not exist. \
             Check --track-id '{}' and --items-dir '{}'.",
            track_dir.display(),
            track_id.as_ref(),
            items_dir.display(),
        ));
    }

    // Use the canonicalized repo root as the trusted_root for symlink guards:
    // `canonicalize()` resolves symlinks and returns the physical path, so
    // `canonical_root` is guaranteed non-symlink and safe as a trusted root.
    let scope_json_path = canonical_root.join("track/review-scope.json");
    let scope_config = load_v2_scope_config(&scope_json_path, track_id, &canonical_root)
        .map_err(|e| format!("load review-scope.json: {e}"))?;

    let review_json_path = track_dir.join("review.json");
    let commit_hash_path = track_dir.join(".commit_hash");
    let review_store = FsReviewStore::new(review_json_path, canonical_root.clone());
    let commit_hash_store = FsCommitHashStore::new(commit_hash_path, canonical_root.clone());

    let base = resolve_diff_base(&commit_hash_store, &git)?;

    Ok((scope_config, review_store, commit_hash_store, base))
}

/// Resolves the diff base commit hash.
fn resolve_diff_base(store: &FsCommitHashStore, git: &SystemGitRepo) -> Result<CommitHash, String> {
    match store.read() {
        Ok(Some(hash)) => return Ok(hash),
        Ok(None) => {}
        Err(e) => {
            eprintln!("[warn] failed to read .commit_hash, falling back to main: {e}");
        }
    }

    let output =
        git.output(&["rev-parse", "main"]).map_err(|e| format!("git rev-parse main: {e}"))?;
    if !output.status.success() {
        return Err("git rev-parse main failed".to_owned());
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    CommitHash::try_new(&sha).map_err(|e| format!("invalid main SHA: {e}"))
}

/// Outcome of a single Codex review run dispatched via `run_codex_review_str`.
///
/// Returned by `run_codex_review_str` so the CLI can emit output without
/// importing `domain::review_v2::Verdict`, `FastVerdict`, or `ReviewOutcome`
/// directly (CN-01 / AC-03).
pub enum CodexReviewOutcome {
    /// The scope had no diff files — review was skipped.
    Skipped { scope_label: String },
    /// A final review completed: verdict JSON to emit and the exit code (0 or 2).
    FinalCompleted { verdict_json: String, exit_code: u8 },
    /// A fast review completed: verdict JSON to emit and the exit code (0 or 2).
    FastCompleted { verdict_json: String, exit_code: u8 },
}

/// Runs the full Codex review cycle from string inputs.
///
/// Encapsulates `TrackId`, `ScopeName`, `RoundType`, `ReviewOutcome`, `Verdict`,
/// `FastVerdict`, `ReviewWriter`, and `ReviewerFinding` conversions so the CLI
/// layer never imports these domain types directly (CN-01 / AC-03).
///
/// Steps performed:
/// 1. Validates `track_id_str` and `group_str` (rejects invalid identifiers).
/// 2. Builds the v2 review composition with the provided `CodexReviewer`.
/// 3. Dispatches the review round (`review` or `fast_review`) per `round_type_str`.
/// 4. Writes the verdict to `review.json` via `ReviewWriter`.
/// 5. Returns `CodexReviewOutcome` describing the result.
///
/// # Errors
/// Returns a human-readable error string on failure at any step.
pub fn run_codex_review_str(
    track_id_str: &str,
    items_dir: &Path,
    group_str: &str,
    round_type_str: &str, // "fast" | "final"
    reviewer: CodexReviewer,
) -> Result<CodexReviewOutcome, String> {
    use domain::review_v2::{
        FastVerdict, MainScopeName, ReviewOutcome, ReviewWriter, ReviewerFinding, ScopeName,
        Verdict,
    };
    use usecase::review_workflow::{
        ReviewFinalPayload, ReviewPayloadVerdict, render_review_payload,
    };

    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("[ERROR] invalid track id: {e}"))?;

    let scope = if group_str == "other" {
        ScopeName::Other
    } else {
        match MainScopeName::new(group_str) {
            Ok(main) => ScopeName::Main(main),
            Err(e) => return Err(format!("[ERROR] invalid scope name: {e}")),
        }
    };

    let comp = build_review_v2_with_reviewer(&track_id, items_dir, reviewer)
        .map_err(|e| format!("[ERROR] v2 composition failed: {e}"))?;

    fn finding_to_payload(f: &ReviewerFinding) -> usecase::review_workflow::ReviewFinding {
        usecase::review_workflow::ReviewFinding {
            message: f.message().to_owned(),
            severity: f.severity().map(str::to_owned),
            file: f.file().map(str::to_owned),
            line: f.line(),
            category: f.category().map(str::to_owned),
        }
    }

    fn render_verdict_final(verdict: &Verdict) -> Result<(String, u8), String> {
        let (payload, exit_code) = match verdict {
            Verdict::ZeroFindings => (
                ReviewFinalPayload {
                    verdict: ReviewPayloadVerdict::ZeroFindings,
                    findings: vec![],
                },
                0u8,
            ),
            Verdict::FindingsRemain(nef) => {
                let findings = nef.as_slice().iter().map(finding_to_payload).collect();
                (
                    ReviewFinalPayload { verdict: ReviewPayloadVerdict::FindingsRemain, findings },
                    2u8,
                )
            }
        };
        let json = render_review_payload(&payload).map_err(|e| format!("[ERROR] {e}"))?;
        Ok((json, exit_code))
    }

    fn render_verdict_fast(verdict: &FastVerdict) -> Result<(String, u8), String> {
        let (payload, exit_code) = match verdict {
            FastVerdict::ZeroFindings => (
                ReviewFinalPayload {
                    verdict: ReviewPayloadVerdict::ZeroFindings,
                    findings: vec![],
                },
                0u8,
            ),
            FastVerdict::FindingsRemain(nef) => {
                let findings = nef.as_slice().iter().map(finding_to_payload).collect();
                (
                    ReviewFinalPayload { verdict: ReviewPayloadVerdict::FindingsRemain, findings },
                    2u8,
                )
            }
        };
        let json = render_review_payload(&payload).map_err(|e| format!("[ERROR] {e}"))?;
        Ok((json, exit_code))
    }

    match round_type_str {
        "final" => match comp.cycle.review(&scope) {
            Ok(ReviewOutcome::Skipped) => {
                Ok(CodexReviewOutcome::Skipped { scope_label: group_str.to_owned() })
            }
            Ok(ReviewOutcome::Reviewed { verdict, hash, .. }) => {
                comp.review_store
                    .write_verdict(&scope, &verdict, &hash)
                    .map_err(|e| format!("[ERROR] record failed: {e}"))?;
                let (json, exit_code) = render_verdict_final(&verdict)?;
                Ok(CodexReviewOutcome::FinalCompleted { verdict_json: json, exit_code })
            }
            Err(e) => Err(format!("[ERROR] {e}")),
        },
        "fast" => match comp.cycle.fast_review(&scope) {
            Ok(ReviewOutcome::Skipped) => {
                Ok(CodexReviewOutcome::Skipped { scope_label: group_str.to_owned() })
            }
            Ok(ReviewOutcome::Reviewed { verdict, hash, .. }) => {
                comp.review_store
                    .write_fast_verdict(&scope, &verdict, &hash)
                    .map_err(|e| format!("[ERROR] record failed: {e}"))?;
                let (json, exit_code) = render_verdict_fast(&verdict)?;
                Ok(CodexReviewOutcome::FastCompleted { verdict_json: json, exit_code })
            }
            Err(e) => Err(format!("[ERROR] {e}")),
        },
        other => Err(format!("[ERROR] unknown round type: '{other}' (expected 'fast' or 'final')")),
    }
}

/// Runs the full Claude review cycle from string inputs.
///
/// Mirrors [`run_codex_review_str`] with `ClaudeReviewer` in place of `CodexReviewer`.
///
/// Encapsulates `TrackId`, `ScopeName`, `RoundType`, `ReviewOutcome`, `Verdict`,
/// `FastVerdict`, `ReviewWriter`, and `ReviewerFinding` conversions so the CLI
/// layer never imports these domain types directly (CN-01 / AC-03).
///
/// Steps performed:
/// 1. Validates `track_id_str` and `group_str` (rejects invalid identifiers).
/// 2. Builds the v2 review composition with the provided `ClaudeReviewer`.
/// 3. Dispatches the review round (`review` or `fast_review`) per `round_type_str`.
/// 4. Writes the verdict to `review.json` via `ReviewWriter`.
/// 5. Returns `CodexReviewOutcome` describing the result.
///
/// # Errors
/// Returns a human-readable error string on failure at any step.
pub fn run_claude_review_str(
    track_id_str: &str,
    items_dir: &Path,
    group_str: &str,
    round_type_str: &str, // "fast" | "final"
    reviewer: ClaudeReviewer,
) -> Result<CodexReviewOutcome, String> {
    use domain::review_v2::{
        FastVerdict, MainScopeName, ReviewOutcome, ReviewWriter, ReviewerFinding, ScopeName,
        Verdict,
    };
    use usecase::review_workflow::{
        ReviewFinalPayload, ReviewPayloadVerdict, render_review_payload,
    };

    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("[ERROR] invalid track id: {e}"))?;

    let scope = if group_str == "other" {
        ScopeName::Other
    } else {
        match MainScopeName::new(group_str) {
            Ok(main) => ScopeName::Main(main),
            Err(e) => return Err(format!("[ERROR] invalid scope name: {e}")),
        }
    };

    let comp = build_review_v2_with_claude_reviewer(&track_id, items_dir, reviewer)
        .map_err(|e| format!("[ERROR] v2 composition failed: {e}"))?;

    fn finding_to_payload(f: &ReviewerFinding) -> usecase::review_workflow::ReviewFinding {
        usecase::review_workflow::ReviewFinding {
            message: f.message().to_owned(),
            severity: f.severity().map(str::to_owned),
            file: f.file().map(str::to_owned),
            line: f.line(),
            category: f.category().map(str::to_owned),
        }
    }

    fn render_verdict_final(verdict: &Verdict) -> Result<(String, u8), String> {
        let (payload, exit_code) = match verdict {
            Verdict::ZeroFindings => (
                ReviewFinalPayload {
                    verdict: ReviewPayloadVerdict::ZeroFindings,
                    findings: vec![],
                },
                0u8,
            ),
            Verdict::FindingsRemain(nef) => {
                let findings = nef.as_slice().iter().map(finding_to_payload).collect();
                (
                    ReviewFinalPayload { verdict: ReviewPayloadVerdict::FindingsRemain, findings },
                    2u8,
                )
            }
        };
        let json = render_review_payload(&payload).map_err(|e| format!("[ERROR] {e}"))?;
        Ok((json, exit_code))
    }

    fn render_verdict_fast(verdict: &FastVerdict) -> Result<(String, u8), String> {
        let (payload, exit_code) = match verdict {
            FastVerdict::ZeroFindings => (
                ReviewFinalPayload {
                    verdict: ReviewPayloadVerdict::ZeroFindings,
                    findings: vec![],
                },
                0u8,
            ),
            FastVerdict::FindingsRemain(nef) => {
                let findings = nef.as_slice().iter().map(finding_to_payload).collect();
                (
                    ReviewFinalPayload { verdict: ReviewPayloadVerdict::FindingsRemain, findings },
                    2u8,
                )
            }
        };
        let json = render_review_payload(&payload).map_err(|e| format!("[ERROR] {e}"))?;
        Ok((json, exit_code))
    }

    match round_type_str {
        "final" => match comp.cycle.review(&scope) {
            Ok(ReviewOutcome::Skipped) => {
                Ok(CodexReviewOutcome::Skipped { scope_label: group_str.to_owned() })
            }
            Ok(ReviewOutcome::Reviewed { verdict, hash, .. }) => {
                comp.review_store
                    .write_verdict(&scope, &verdict, &hash)
                    .map_err(|e| format!("[ERROR] record failed: {e}"))?;
                let (json, exit_code) = render_verdict_final(&verdict)?;
                Ok(CodexReviewOutcome::FinalCompleted { verdict_json: json, exit_code })
            }
            Err(e) => Err(format!("[ERROR] {e}")),
        },
        "fast" => match comp.cycle.fast_review(&scope) {
            Ok(ReviewOutcome::Skipped) => {
                Ok(CodexReviewOutcome::Skipped { scope_label: group_str.to_owned() })
            }
            Ok(ReviewOutcome::Reviewed { verdict, hash, .. }) => {
                comp.review_store
                    .write_fast_verdict(&scope, &verdict, &hash)
                    .map_err(|e| format!("[ERROR] record failed: {e}"))?;
                let (json, exit_code) = render_verdict_fast(&verdict)?;
                Ok(CodexReviewOutcome::FastCompleted { verdict_json: json, exit_code })
            }
            Err(e) => Err(format!("[ERROR] {e}")),
        },
        other => Err(format!("[ERROR] unknown round type: '{other}' (expected 'fast' or 'final')")),
    }
}

/// Appends a scope-specific severity policy reference section to `prompt`
/// when the given scope has a `briefing_file` configured and the path is safe
/// to inject.
///
/// This is the string-accepting variant of the CLI `append_scope_briefing_reference`
/// helper. It loads scope config from the given track and items_dir, then checks
/// the configured briefing file for `scope_name`. No I/O beyond config loading.
///
/// String-accepting so the CLI never imports `domain::ScopeName` or
/// `domain::ReviewScopeConfig` (CN-01 / AC-03).
///
/// # Errors
/// Returns an error string if the track ID is invalid or the scope config
/// cannot be loaded.
pub fn append_scope_briefing_reference_str(
    prompt: &mut String,
    scope_name: &str,
    track_id_str: &str,
    items_dir: &Path,
    is_safe_path_fn: impl Fn(&str) -> bool,
) -> Result<(), String> {
    use domain::review_v2::{MainScopeName, ScopeName};

    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    let scope_config = load_scope_config_only(&track_id, items_dir)?;

    let scope = if scope_name == "other" {
        ScopeName::Other
    } else {
        match MainScopeName::new(scope_name) {
            Ok(main) => ScopeName::Main(main),
            Err(e) => {
                return Err(format!("invalid scope name '{scope_name}': {e}"));
            }
        }
    };

    let Some(briefing_path) = scope_config.briefing_file_for_scope(&scope) else {
        return Ok(());
    };

    if !is_safe_path_fn(briefing_path) {
        return Ok(());
    }

    prompt.push_str("\n\n## Scope-specific severity policy\n\n");
    prompt.push_str(&format!(
        "このレビューの scope は `{scope_name}` である。\
         以下の scope 固有 severity policy を **必ず先に Read ツールで読み込み**、\
         その方針に従って findings を選別すること:\n\n\
         - `{briefing_path}`",
    ));

    Ok(())
}

/// Validates a track ID string without exposing `domain::TrackId` to the caller.
///
/// Returns `Ok(())` if the string is a valid track ID, or `Err(reason)` if not.
/// Used by the CLI composition root to pre-validate `--track-id` arguments
/// without importing `domain::TrackId` (CN-01 / AC-03).
///
/// # Errors
/// Returns a string describing why the track ID is invalid.
pub fn validate_track_id_str(track_id_str: &str) -> Result<(), String> {
    TrackId::try_new(track_id_str).map(|_| ()).map_err(|e| e.to_string())
}

/// Validates a review group name string without exposing domain types to the caller.
///
/// Returns `Ok(())` if the string is a valid `ReviewGroupName`, or `Err(reason)`.
/// Used by the CLI composition root to pre-validate `--group` arguments
/// without importing `domain::ReviewGroupName` (CN-01 / AC-03).
///
/// # Errors
/// Returns a string describing why the group name is invalid.
pub fn validate_review_group_name_str(group_name: &str) -> Result<(), String> {
    domain::ReviewGroupName::try_new(group_name).map(|_| ()).map_err(|e| e.to_string())
}

/// Returns the configured briefing file path (as `Option<String>`) for the
/// given scope in the given track's scope configuration.
///
/// This lets the CLI layer query whether a scope has a briefing configured
/// without importing `domain::review_v2::ScopeName` or `ReviewScopeConfig`
/// (CN-01 / AC-03).
///
/// Returns `Ok(None)` when the scope has no briefing configured or the scope
/// name is "other" (the implicit scope never receives scope-specific briefings).
/// Returns `Err` if the track ID or scope config cannot be loaded.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn get_briefing_for_scope_str(
    scope_name: &str,
    track_id_str: &str,
    items_dir: &Path,
) -> Result<Option<String>, String> {
    use domain::review_v2::{MainScopeName, ScopeName};

    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    let scope_config = load_scope_config_only(&track_id, items_dir)?;

    let scope = if scope_name == "other" {
        ScopeName::Other
    } else {
        match MainScopeName::new(scope_name) {
            Ok(main) => ScopeName::Main(main),
            Err(e) => {
                return Err(format!("invalid scope name '{scope_name}': {e}"));
            }
        }
    };

    Ok(scope_config.briefing_file_for_scope(&scope).map(str::to_owned))
}

/// Runs the full check-approved operation from string inputs and returns a
/// `ReviewApprovalOutput` DTO (usecase-owned, no domain types exposed).
///
/// Encapsulates `TrackId`, `ReviewExistsPort`, and `ReviewApprovalVerdict`
/// conversions so that the CLI layer never imports domain review types directly
/// (CN-01 / AC-03).
///
/// # Errors
/// Returns `ReviewCheckApprovedError` on track ID validation, store, or
/// evaluation failures.
pub fn check_approved_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError> {
    use domain::review_v2::ReviewApprovalVerdict;

    let track_id = TrackId::try_new(track_id_str)
        .map_err(|e| ReviewCheckApprovedError::InvalidTrackId(e.to_string()))?;

    let comp = build_review_v2(&track_id, items_dir)
        .map_err(ReviewCheckApprovedError::ReviewStoreError)?;

    let review_json_exists = comp
        .review_store
        .review_json_exists()
        .map_err(|e| ReviewCheckApprovedError::ReviewStoreError(format!("{e}")))?;

    let verdict = comp
        .cycle
        .evaluate_approval(&comp.review_store, review_json_exists)
        .map_err(|e| ReviewCheckApprovedError::EvaluationFailed(e.to_string()))?;

    Ok(match verdict {
        ReviewApprovalVerdict::Approved => ReviewApprovalOutput {
            decision: ReviewApprovalDecision::Approved,
            bypass_scope_count: None,
            blocked_scopes: Vec::new(),
        },
        ReviewApprovalVerdict::ApprovedWithBypass { not_started_count } => ReviewApprovalOutput {
            decision: ReviewApprovalDecision::ApprovedWithBypass,
            bypass_scope_count: Some(not_started_count),
            blocked_scopes: Vec::new(),
        },
        ReviewApprovalVerdict::Blocked { required_scopes } => ReviewApprovalOutput {
            decision: ReviewApprovalDecision::Blocked,
            bypass_scope_count: None,
            blocked_scopes: required_scopes.iter().map(|s| s.to_string()).collect(),
        },
    })
}

/// Extracts the finding count from a verdict JSON string produced by
/// [`render_review_payload`].
///
/// Parses the JSON `"findings"` array and returns its length. Returns `0` when
/// the JSON cannot be parsed (fail-safe: the caller still has the raw JSON in
/// `summary`).
fn count_findings_in_verdict_json(verdict_json: &str) -> usize {
    serde_json::from_str::<serde_json::Value>(verdict_json)
        .ok()
        .and_then(|v| v.get("findings").and_then(|f| f.as_array()).map(Vec::len))
        .unwrap_or(0)
}

/// Constructs an `Arc<dyn RunReviewService>` that the CLI can call without
/// importing infrastructure or domain types.
///
/// Returns a `RunReviewInteractor` whose closure builds a `CodexReviewer` from
/// the command fields and dispatches the review via [`run_codex_review_str`].
///
/// # Purpose
///
/// This factory gives the CLI a usecase-service-trait handle rather than a
/// concrete `ReviewV2CompositionWithCodex` struct, satisfying the CN-01 / AC-03
/// wiring requirement: the CLI composition root wires through
/// `Arc<dyn RunReviewService>` rather than touching concrete infrastructure
/// adapters directly.
#[must_use]
pub fn build_run_review_service() -> std::sync::Arc<dyn RunReviewService> {
    use std::sync::Arc;
    use std::time::Duration;

    let run_fn = Arc::new(|cmd: RunReviewCommand| {
        // Set `scope_label` so the reviewer prompt includes `Review scope: <group>`.
        let reviewer = CodexReviewer::new(
            cmd.model.clone(),
            Duration::from_secs(cmd.timeout_seconds),
            cmd.base_prompt.clone(),
        )
        .with_scope_label(cmd.group.clone());
        run_codex_review_str(&cmd.track_id, &cmd.items_dir, &cmd.group, &cmd.round_type, reviewer)
            .map_err(RunReviewError::CompositionFailed)
            .map(|outcome| match outcome {
                CodexReviewOutcome::Skipped { .. } => RunReviewOutput {
                    verdict_kind: "skipped".to_owned(),
                    skipped: true,
                    finding_count: 0,
                    summary: None,
                },
                CodexReviewOutcome::FinalCompleted { verdict_json, exit_code } => {
                    let finding_count = count_findings_in_verdict_json(&verdict_json);
                    RunReviewOutput {
                        verdict_kind: if exit_code == 0 {
                            "approved".to_owned()
                        } else {
                            "rejected".to_owned()
                        },
                        skipped: false,
                        finding_count,
                        summary: Some(verdict_json),
                    }
                }
                CodexReviewOutcome::FastCompleted { verdict_json, exit_code } => {
                    let finding_count = count_findings_in_verdict_json(&verdict_json);
                    RunReviewOutput {
                        verdict_kind: if exit_code == 0 {
                            "approved".to_owned()
                        } else {
                            "rejected".to_owned()
                        },
                        skipped: false,
                        finding_count,
                        summary: Some(verdict_json),
                    }
                }
            })
    });

    Arc::new(RunReviewInteractor::new(run_fn))
}

/// Constructs an `Arc<dyn ReviewCheckApprovedService>` that the CLI can call
/// without importing infrastructure or domain types.
///
/// Returns a `ReviewCheckApprovedInteractor` whose closure delegates to
/// [`check_approved_str`] to perform the full domain + I/O operation.
///
/// # Purpose
///
/// This factory gives the CLI a usecase-service-trait handle rather than a
/// concrete `ReviewV2Composition` struct, satisfying the CN-01 / AC-03 wiring
/// requirement: the CLI composition root wires through
/// `Arc<dyn ReviewCheckApprovedService>` rather than touching concrete
/// infrastructure adapters directly.
#[must_use]
pub fn build_check_approved_service() -> std::sync::Arc<dyn ReviewCheckApprovedService> {
    use std::sync::Arc;
    Arc::new(ReviewCheckApprovedInteractor::new(|track_id, items_dir| {
        check_approved_str(track_id, items_dir)
    }))
}

/// Renders the `sotp review results` output as a string, given string-typed parameters.
///
/// Performs all domain operations (build composition, fetch states, evaluate approval,
/// read rounds) internally so that `commands/review/results.rs` never imports domain
/// types directly (CN-01 / AC-03).
///
/// # Parameters
/// - `scope_filter` — optional scope name to filter displayed scopes
/// - `limit` — `None` = state summary only (equivalent to `--limit 0`);
///   `Some(u32::MAX)` = all rounds; `Some(n)` = up to `n` rounds
/// - `round_type` — round-type filter string: `"any"` | `"fast"` | `"final"`
/// - `no_hint` — suppress the commit hint line
///
/// # Errors
/// Returns a human-readable error string on any I/O or domain failure.
pub fn render_review_results_str(
    track_id_str: &str,
    items_dir: &Path,
    scope_filter: Option<&str>,
    limit: Option<u32>,
    round_type: &str,
    no_hint: bool,
) -> Result<String, String> {
    use domain::review_v2::{
        NotRequiredReason, ReviewApprovalVerdict, ReviewExistsPort as _, ReviewReader, ReviewState,
        ReviewerFinding, RoundType, ScopeName, ScopeRound, Verdict,
    };
    use std::collections::HashMap;
    use std::fmt::Write as _;

    let track_id = TrackId::try_new(track_id_str).map_err(|e| format!("invalid track id: {e}"))?;
    let comp = build_review_v2(&track_id, items_dir)?;

    let states = comp
        .cycle
        .get_review_states(&comp.review_store)
        .map_err(|e| format!("failed to get review states: {e}"))?;

    let review_json_exists = comp
        .review_store
        .review_json_exists()
        .map_err(|e| format!("failed to check review.json existence: {e}"))?;

    let approval_verdict = comp
        .cycle
        .evaluate_approval(&comp.review_store, review_json_exists)
        .map_err(|e| format!("failed to evaluate approval: {e}"))?;

    // Sort scope universe alphabetically.
    let mut scope_universe: Vec<ScopeName> = states.keys().cloned().collect();
    scope_universe.sort_by_key(ToString::to_string);

    // Apply optional scope filter.
    let displayed_scopes: Vec<ScopeName> = if let Some(name) = scope_filter {
        if let Some(scope) = scope_universe.iter().find(|s| s.to_string() == name) {
            vec![scope.clone()]
        } else {
            return Err(format!("scope '{name}' is not defined for this track"));
        }
    } else {
        scope_universe.clone()
    };

    // Load rounds per scope (only when limit > 0).
    let rounds_per_scope: HashMap<ScopeName, Vec<ScopeRound>> = if limit.is_none() {
        HashMap::new()
    } else {
        let mut map = HashMap::new();
        for scope in &displayed_scopes {
            let rounds = comp
                .review_store
                .read_all_rounds(scope)
                .map_err(|e| format!("failed to read rounds for {scope}: {e}"))?;
            map.insert(scope.clone(), rounds);
        }
        map
    };

    // --- Rendering ---

    let is_round_type_fast = round_type == "fast";
    let is_round_type_final = round_type == "final";

    fn round_type_label(rt: RoundType) -> &'static str {
        match rt {
            RoundType::Fast => "fast",
            RoundType::Final => "final",
        }
    }

    fn verdict_label(v: &Verdict) -> &'static str {
        match v {
            Verdict::ZeroFindings => "zero_findings",
            Verdict::FindingsRemain(_) => "findings_remain",
        }
    }

    fn state_line_suffix(rounds: &[ScopeRound]) -> String {
        rounds.last().map_or_else(String::new, |latest| {
            format!(
                "  {}@{} {}",
                match latest.round_type {
                    RoundType::Fast => "fast",
                    RoundType::Final => "final",
                },
                latest.at,
                match &latest.verdict {
                    Verdict::ZeroFindings => "zero_findings",
                    Verdict::FindingsRemain(_) => "findings_remain",
                }
            )
        })
    }

    fn render_findings_block(out: &mut String, findings: &[ReviewerFinding]) {
        if findings.is_empty() {
            let _ = writeln!(out, "    findings: zero_findings");
            return;
        }
        let _ = writeln!(out, "    findings:");
        for finding in findings {
            let severity = finding.severity().unwrap_or("-");
            let location = match (finding.file(), finding.line()) {
                (Some(path), Some(line)) => format!(" ({path}:{line})"),
                (Some(path), None) => format!(" ({path})"),
                (None, _) => String::new(),
            };
            let _ = writeln!(
                out,
                "      - [{severity}] {message}{location}",
                message = finding.message()
            );
            if let Some(category) = finding.category() {
                let _ = writeln!(out, "        category: {category}");
            }
        }
    }

    // Selects which rounds to display based on limit and round_type filter.
    // Returns references into the provided `rounds` slice, newest first.
    fn select_rounds_inner<'a>(
        rounds: &'a [ScopeRound],
        limit: Option<u32>,
        is_fast: bool,
        is_final: bool,
    ) -> Vec<&'a ScopeRound> {
        let Some(n) = limit else {
            return Vec::new();
        };
        let mut filtered: Vec<&'a ScopeRound> = rounds
            .iter()
            .rev()
            .filter(|r| {
                if is_fast {
                    matches!(r.round_type, RoundType::Fast)
                } else if is_final {
                    matches!(r.round_type, RoundType::Final)
                } else {
                    true
                }
            })
            .collect();
        if n != u32::MAX {
            filtered.truncate(n as usize);
        }
        filtered
    }

    let mut out = String::new();
    let _ = writeln!(out, "Review results (v2 scope-based):");
    let _ = writeln!(out, "Diff base: {}", comp.base);
    let _ = writeln!(out);

    let mut approved_count = 0usize;
    let mut empty_count = 0usize;
    let mut required_count = 0usize;

    for scope in &displayed_scopes {
        let state = match states.get(scope) {
            Some(s) => s,
            None => continue,
        };
        let indicator = match state {
            ReviewState::Required(_) => {
                required_count += 1;
                "[-]"
            }
            ReviewState::NotRequired(NotRequiredReason::Empty) => {
                empty_count += 1;
                "[.]"
            }
            ReviewState::NotRequired(NotRequiredReason::ZeroFindings) => {
                approved_count += 1;
                "[+]"
            }
        };
        let scope_rounds = rounds_per_scope.get(scope).map(Vec::as_slice).unwrap_or(&[]);
        let suffix = state_line_suffix(scope_rounds);
        let _ = writeln!(out, "  {indicator} {scope}: {state}{suffix}");

        let displayed_rounds =
            select_rounds_inner(scope_rounds, limit, is_round_type_fast, is_round_type_final);
        if let Some((latest, history)) = displayed_rounds.split_first() {
            render_findings_block(&mut out, latest.findings.as_slice());
            if !history.is_empty() {
                let _ = writeln!(out, "    history (newer first, up to --limit):");
                for round in history {
                    let _ = writeln!(
                        out,
                        "      - {}@{} {}",
                        round_type_label(round.round_type),
                        round.at,
                        verdict_label(&round.verdict)
                    );
                }
            }
        }
    }

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Summary: {approved_count} approved, {empty_count} empty, {required_count} required, {} total",
        displayed_scopes.len()
    );

    let hint_should_emit =
        matches!(approval_verdict, ReviewApprovalVerdict::Approved) && review_json_exists;
    if !no_hint && hint_should_emit {
        let _ =
            writeln!(out, "hint: review approved — run /track:commit <message> to record changes.");
    }

    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::process::Command;
    use std::time::Duration;

    use super::*;
    use crate::review_v2::ClaudeReviewer;

    // Mutex so tests that mutate cwd do not race.
    static ENV_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    fn env_lock() -> &'static std::sync::Mutex<()> {
        ENV_LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    /// Guard that restores the working directory when dropped.
    struct CwdGuard {
        original: std::path::PathBuf,
    }

    impl CwdGuard {
        fn change_to(path: &std::path::Path) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { original }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    /// Sets up a minimal git repo with v2 review-scope.json for testing.
    ///
    /// Creates two commits so that the diff base (first commit SHA) differs from HEAD.
    /// Returns the SHA of the first commit, which callers write to `.commit_hash` so
    /// that the diff is non-empty and review is not skipped.
    fn setup_test_git_repo(root: &std::path::Path) -> String {
        let run =
            |args: &[&str]| Command::new("git").args(args).current_dir(root).output().unwrap();
        run(&["init", "-b", "main"]);
        run(&["config", "user.email", "test@test.com"]);
        run(&["config", "user.name", "Test"]);
        let track_dir = root.join("track");
        std::fs::create_dir_all(&track_dir).unwrap();
        // `infra` scope matches files under `src/`.
        std::fs::write(
            track_dir.join("review-scope.json"),
            r#"{"version": 2, "groups": {"infra": {"patterns": ["src/**"]}}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(root.join("track/items")).unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "base commit"]);

        // Record the first commit SHA so callers can write it to `.commit_hash`.
        let sha_out =
            Command::new("git").args(["rev-parse", "HEAD"]).current_dir(root).output().unwrap();
        let base_sha = String::from_utf8_lossy(&sha_out.stdout).trim().to_owned();

        // Second commit: add `src/lib.rs` so the diff against base is non-empty.
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "// test\n").unwrap();
        run(&["add", "src/lib.rs"]);
        run(&["commit", "-m", "add src/lib.rs"]);

        base_sha
    }

    fn make_claude_reviewer() -> ClaudeReviewer {
        ClaudeReviewer::new("claude-opus-4-7", Duration::from_secs(10), "Review.")
    }

    #[test]
    fn test_run_claude_review_str_rejects_invalid_track_id() {
        let result = run_claude_review_str(
            "../evil",
            std::path::Path::new("track/items"),
            "infrastructure",
            "fast",
            make_claude_reviewer(),
        );
        assert!(result.is_err(), "invalid track id must be rejected");
        let msg = result.err().unwrap();
        assert!(msg.contains("[ERROR]"), "error message must contain [ERROR] prefix: {msg}");
    }

    #[test]
    fn test_run_claude_review_str_rejects_unknown_round_type() {
        // The "unknown round type" branch is reached only after the composition builds
        // successfully. Use a real git repo + track dir to exercise it.
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let base_sha = setup_test_git_repo(dir.path());
        let _cwd = CwdGuard::change_to(dir.path());

        let items_dir = dir.path().join("track/items");
        let track_id = "my-test-track-2026";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        // Write diff base so the composition builds successfully.
        std::fs::write(track_dir.join(".commit_hash"), &base_sha).unwrap();

        let result = run_claude_review_str(
            track_id,
            &items_dir,
            "infra",
            "bogus-round",
            make_claude_reviewer(),
        );
        assert!(result.is_err(), "unknown round type must be rejected");
        let msg = result.err().unwrap();
        assert!(
            msg.contains("unknown round type"),
            "error must mention 'unknown round type': {msg}"
        );
    }

    /// Writes an executable shell script at `path` that outputs the given JSON envelope on stdout.
    #[cfg(unix)]
    fn write_fake_claude_script(path: &std::path::Path, envelope_json: &str) {
        use std::os::unix::fs::PermissionsExt;
        let content = format!(
            "#!/bin/sh\nprintf '%s\\n' '{}'\nexit 0\n",
            envelope_json.replace('\'', "'\\''")
        );
        std::fs::write(path, content).unwrap();
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }

    /// Builds a `ClaudeReviewer` that uses the given binary path instead of `claude`.
    #[cfg(unix)]
    fn make_reviewer_with_bin(bin: impl Into<std::ffi::OsString>) -> ClaudeReviewer {
        ClaudeReviewer::new("claude-opus-4-7", Duration::from_secs(10), "Review.").with_bin(bin)
    }

    #[cfg(unix)]
    #[test]
    fn test_run_claude_review_str_fast_zero_findings_writes_verdict_and_returns_outcome() {
        // AC-03 / write-first / fail-closed: after a zero-findings verdict the verdict is
        // written to review.json before being returned, and a FastCompleted outcome is produced.
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let base_sha = setup_test_git_repo(dir.path());
        let _cwd = CwdGuard::change_to(dir.path());

        let script = dir.path().join("fake-claude.sh");
        write_fake_claude_script(
            &script,
            r#"{"type":"result","structured_output":{"verdict":"zero_findings","findings":[]}}"#,
        );

        let items_dir = dir.path().join("track/items");
        let track_id = "my-test-track-2026";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        // Write diff base pointing to the first commit so `src/lib.rs` appears in the diff.
        std::fs::write(track_dir.join(".commit_hash"), &base_sha).unwrap();

        let reviewer = make_reviewer_with_bin(&script);
        let result = run_claude_review_str(track_id, &items_dir, "infra", "fast", reviewer);

        let outcome = result.expect("fast zero-findings review must succeed");
        assert!(
            matches!(outcome, CodexReviewOutcome::FastCompleted { exit_code: 0, .. }),
            "expected FastCompleted with exit_code 0"
        );

        // write-first: review.json must have been written (fail-closed guarantee).
        let review_json = track_dir.join("review.json");
        assert!(review_json.exists(), "review.json must be written (write-first contract)");
    }

    #[cfg(unix)]
    #[test]
    fn test_run_claude_review_str_fast_findings_remain_writes_verdict_and_returns_outcome() {
        // AC-03: findings_remain case also writes review.json (write-first / fail-closed).
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let base_sha = setup_test_git_repo(dir.path());
        let _cwd = CwdGuard::change_to(dir.path());

        let script = dir.path().join("fake-claude-findings.sh");
        write_fake_claude_script(
            &script,
            r#"{"type":"result","structured_output":{"verdict":"findings_remain","findings":[{"message":"A finding","severity":"P2","file":"src/lib.rs","line":1,"category":"style"}]}}"#,
        );

        let items_dir = dir.path().join("track/items");
        let track_id = "my-test-track-2026";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        // Write diff base pointing to the first commit so `src/lib.rs` appears in the diff.
        std::fs::write(track_dir.join(".commit_hash"), &base_sha).unwrap();

        let reviewer = make_reviewer_with_bin(&script);
        let result = run_claude_review_str(track_id, &items_dir, "infra", "fast", reviewer);

        let outcome = result.expect("fast findings_remain review must succeed");
        assert!(
            matches!(outcome, CodexReviewOutcome::FastCompleted { exit_code: 2, .. }),
            "expected FastCompleted with exit_code 2"
        );

        // write-first: review.json must have been written before returning the outcome.
        let review_json = track_dir.join("review.json");
        assert!(review_json.exists(), "review.json must be written (write-first contract)");
    }

    #[cfg(unix)]
    #[test]
    fn test_run_claude_review_str_final_zero_findings_writes_verdict_and_returns_outcome() {
        // AC-03 / final-round path: a zero-findings final verdict writes review.json
        // and returns FinalCompleted with exit_code 0.
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let base_sha = setup_test_git_repo(dir.path());
        let _cwd = CwdGuard::change_to(dir.path());

        let script = dir.path().join("fake-claude-final.sh");
        write_fake_claude_script(
            &script,
            r#"{"type":"result","structured_output":{"verdict":"zero_findings","findings":[]}}"#,
        );

        let items_dir = dir.path().join("track/items");
        let track_id = "my-test-track-final-2026";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join(".commit_hash"), &base_sha).unwrap();

        let reviewer = make_reviewer_with_bin(&script);
        let result = run_claude_review_str(track_id, &items_dir, "infra", "final", reviewer);

        let outcome = result.expect("final zero-findings review must succeed");
        assert!(
            matches!(outcome, CodexReviewOutcome::FinalCompleted { exit_code: 0, .. }),
            "expected FinalCompleted with exit_code 0"
        );

        // write-first: review.json must have been written (fail-closed guarantee).
        let review_json = track_dir.join("review.json");
        assert!(review_json.exists(), "review.json must be written (write-first contract)");
    }

    #[test]
    fn test_build_review_v2_with_claude_reviewer_str_rejects_invalid_track_id() {
        // build_review_v2_with_claude_reviewer_str validates track_id before any I/O.
        let result = build_review_v2_with_claude_reviewer_str(
            "../evil",
            std::path::Path::new("track/items"),
            make_claude_reviewer(),
        );
        assert!(result.is_err(), "invalid track id must be rejected");
        // Use .err().unwrap() to extract the error string without requiring T: Debug.
        let msg = result.err().unwrap();
        assert!(
            msg.contains("invalid --track-id"),
            "error must mention invalid --track-id, got: {msg}"
        );
    }

    #[test]
    fn test_build_review_v2_with_claude_reviewer_rejects_missing_track_dir() {
        // build_review_v2_with_claude_reviewer rejects a well-formed track_id when
        // the track directory does not exist.
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        setup_test_git_repo(dir.path());
        let _cwd = CwdGuard::change_to(dir.path());

        let items_dir = dir.path().join("track/items");
        let track_id = domain::TrackId::try_new("missing-track-2026").unwrap();
        // Deliberately do NOT create track/items/missing-track-2026.

        let result =
            build_review_v2_with_claude_reviewer(&track_id, &items_dir, make_claude_reviewer());
        assert!(result.is_err(), "missing track directory must be rejected");
        // Use .err().unwrap() to extract the error string without requiring T: Debug.
        let msg = result.err().unwrap();
        assert!(
            msg.contains("does not exist"),
            "error must mention missing track directory, got: {msg}"
        );
    }
}
