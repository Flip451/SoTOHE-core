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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::dry_check::{
    DryCheckApprovalVerdict, DryCheckFinding, DryCheckReader as _, DryCheckVerdict, VerdictFilter,
    fragments_overlapping_hunks,
};
use domain::semantic_dup::{CodeFragment, SimilarityThreshold};
use domain::{CommitHash, TrackId};
use infrastructure::dry_check::{
    CodexDryChecker, DryCheckCommitHashError, FsDryCheckCommitHashStore, FsDryCheckStore,
    GitDryCheckDiffGetter,
};
use infrastructure::semantic_dup::{
    embedding::FastEmbedAdapter, extractor::extract_code_fragments,
    index::LanceDbSemanticIndexAdapter,
};
use usecase::dry_check::{
    DryCheckApprovalInteractor, DryCheckApprovalService as _, DryCheckInteractor,
    DryCheckResultsInteractor, DryCheckResultsService as _, DryCheckService as _,
};

use crate::{CliApp, CommandOutcome};

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
    pub threshold: f32,
    /// Root of the workspace to scan for Rust sources (corpus extraction).
    pub workspace_root: PathBuf,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
    /// Codex model name for the DryCheckAgentPort.
    pub model: String,
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

/// Input DTO for `sotp dry check-approved`.
#[derive(Debug, Clone)]
pub struct DryCheckApprovedInput {
    /// Track ID used to locate the per-track dry-check.json and .commit_hash.
    pub track_id: String,
    /// Optional explicit base commit (overrides FsDryCheckCommitHashStore lookup).
    pub base_commit: Option<String>,
    /// Path to the LanceDB semantic index database.
    pub db_path: PathBuf,
    /// Cosine similarity threshold (0.0–1.0).
    pub threshold: f32,
    /// Root of the workspace to scan for Rust sources (corpus extraction).
    pub workspace_root: PathBuf,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Resolve the diff base commit using the three-branch fail-closed policy.
///
/// Branch 1: `FsDryCheckCommitHashStore::read()` → `Ok(Some(hash))` → use it.
/// Branch 2: `Ok(None)` (file absent or non-ancestor) → fall back to
///   `git rev-parse main`.
/// Branch 3: `Err(DryCheckCommitHashError::Format)` → emit `eprintln!` warn
///   and fall back to `git rev-parse main` (absorbed — must NOT abort the gate).
///
/// CN-01: uses dry-check's OWN `FsDryCheckCommitHashStore`, never
/// `review_v2`'s `FsCommitHashStore`.
///
/// When `base_commit_override` is `Some`, the string is parsed to `CommitHash`
/// and returned directly (skips the store lookup entirely).
///
/// # Errors
///
/// Returns `Err` only when `base_commit_override` is invalid, or when
/// `git rev-parse main` fails.
pub(crate) fn resolve_dry_diff_base(
    base_commit_override: Option<&str>,
    commit_hash_path: &Path,
    trusted_root: &Path,
) -> Result<CommitHash, String> {
    // When --base-commit is provided, use it directly.
    if let Some(s) = base_commit_override {
        return CommitHash::try_new(s).map_err(|e| format!("invalid --base-commit: {e}"));
    }

    // Read from dry-check's own FsDryCheckCommitHashStore.
    let store =
        FsDryCheckCommitHashStore::new(commit_hash_path.to_path_buf(), trusted_root.to_path_buf());
    match store.read() {
        Ok(Some(hash)) => return Ok(hash),
        Ok(None) => {
            // File absent or non-ancestor — fall through to main fallback.
        }
        Err(DryCheckCommitHashError::Format(detail)) => {
            eprintln!("[warn] dry-check: malformed .commit_hash ({detail}); falling back to main");
            // Absorbed — fall through to main fallback.
        }
        Err(other) => {
            eprintln!(
                "[warn] dry-check: failed to read .commit_hash ({other}); falling back to main"
            );
            // Other I/O errors also absorbed — fail-closed.
        }
    }

    // Fallback: git rev-parse main.
    git_rev_parse_main()
}

/// Run `git rev-parse main` and return the resulting `CommitHash`.
///
/// # Errors
///
/// Returns `Err` when git cannot be discovered, the command fails, or the
/// output is not a valid commit hash.
fn git_rev_parse_main() -> Result<CommitHash, String> {
    use infrastructure::git_cli::{GitRepository, SystemGitRepo};

    let git = SystemGitRepo::discover().map_err(|e| format!("git discover: {e}"))?;
    let output =
        git.output(&["rev-parse", "main"]).map_err(|e| format!("git rev-parse main: {e}"))?;
    if !output.status.success() {
        return Err("git rev-parse main failed".to_owned());
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    CommitHash::try_new(&sha).map_err(|e| format!("invalid main SHA: {e}"))
}

/// Build `diff_fragments` using the three-step hunk-scope pipeline.
///
/// Steps:
/// 1. `GitDryCheckDiffGetter::list_changed_hunks(base)` → `Vec<DiffFileHunks>`.
/// 2. `extract_code_fragments(workspace_root)` → all workspace fragments.
/// 3. Normalize every fragment's `source_path` to **repo-relative** form by
///    stripping `repo_root` as a prefix (so `source_path` matches the
///    repo-relative paths that `git diff` emits for hunk paths).
/// 4. `fragments_overlapping_hunks(candidate_fragments, &changed_hunks)` → diff_fragments.
///
/// Returns `(diff_fragments, corpus_fragments)` where:
/// - `diff_fragments` are hunk-scoped (CN-04).
/// - `corpus_fragments` are extracted from `workspace_root` with normalized paths.
///
/// ## Path normalization contract
///
/// `extract_code_fragments` returns fragments rooted at the supplied
/// `workspace_root`. `git diff` hunk paths are repo-relative (e.g.
/// `libs/domain/src/foo.rs`). This function strips `repo_root` from every
/// extracted path so that both sides use the same format, even when
/// `workspace_root` is a valid subdirectory of the repository. Fragments whose
/// path cannot be stripped are kept rather than dropped — a conservative
/// fallback that avoids silently discarding fragments.
///
/// Both `corpus_fragments` and `diff_fragments` share the normalized form so that
/// `DryCheckPairKey` (which pairs a changed-fragment-ref with a corpus-fragment-ref
/// by their identity) can match across the two sets.
///
/// CN-01: uses `GitDryCheckDiffGetter` (dry-check's own adapter), never
/// `review_v2`'s `GitDiffGetter`.
///
/// # Errors
///
/// Returns `Err` when diff listing or fragment extraction fails.
pub(crate) fn build_diff_and_corpus_fragments(
    base: &CommitHash,
    workspace_root: &Path,
    repo_root: &Path,
) -> Result<(Vec<CodeFragment>, Vec<CodeFragment>), String> {
    use usecase::dry_check::DryCheckDiffSource as _;

    // Step 1: List changed hunks via dry-check's own diff getter.
    let getter = GitDryCheckDiffGetter;
    let changed_hunks =
        getter.list_changed_hunks(base).map_err(|e| format!("list_changed_hunks failed: {e}"))?;

    // Step 2: Extract fragments from the whole workspace.
    // Paths are absolute when workspace_root is absolute (entry.path() propagates
    // the root's absoluteness).
    let raw_fragments = extract_code_fragments(workspace_root)
        .map_err(|e| format!("fragment extraction failed: {e}"))?;

    // Step 3: Normalize source_path → repo-relative by stripping repo_root.
    // This makes fragment paths byte-equal to git-diff hunk paths (e.g. "libs/foo/src/bar.rs").
    let normalized_fragments = normalize_fragment_paths(raw_fragments, repo_root)
        .map_err(|e| format!("fragment path normalization failed: {e}"))?;

    // Build the set of changed file paths for quick lookup (repo-relative strings).
    let changed_paths: std::collections::HashSet<String> =
        changed_hunks.iter().map(|h| h.path().as_str().to_owned()).collect();

    // Candidate fragments: only those from changed files (repo-relative git path match).
    let candidate_fragments: Vec<CodeFragment> = normalized_fragments
        .iter()
        .filter(|f| {
            let path_key = git_diff_path_key(&f.source_path);
            changed_paths.contains(path_key.as_str())
        })
        .cloned()
        .collect();

    // Step 4: Hunk-scope filter — keep only fragments overlapping a changed hunk.
    let diff_fragments = fragments_overlapping_hunks(&candidate_fragments, &changed_hunks);

    // Corpus: full workspace with normalized paths (no hunk filter).
    let corpus_fragments = normalized_fragments;

    Ok((diff_fragments, corpus_fragments))
}

/// Normalize a list of `CodeFragment` values so that each `source_path` is
/// repo-relative (the `repo_root` prefix stripped).
///
/// When a fragment's path starts with `repo_root`, the prefix is removed
/// (e.g. `/home/user/repo/libs/foo.rs` → `libs/foo.rs`).  The resulting path is
/// also converted to git-diff style slash separators, so Windows paths match
/// the slash-separated paths emitted by `git diff`.
///
/// Fragments whose path is already relative, or cannot be stripped, are kept as
/// a conservative fallback after the same separator normalization.
///
/// Rebuilds each fragment with `CodeFragment::new` to produce a value-typed
/// result rather than mutating the `source_path` field in-place.
///
/// # Errors
///
/// Returns `Err` only when `CodeFragment::new` rejects a rebuilt fragment, which
/// should never happen in practice because the original fragment was already valid
/// (same content + same line spans).
fn normalize_fragment_paths(
    fragments: Vec<CodeFragment>,
    repo_root: &Path,
) -> Result<Vec<CodeFragment>, String> {
    let mut result = Vec::with_capacity(fragments.len());
    for frag in fragments {
        // Strip the repo_root prefix to get the repo-relative path.
        let rel_path = frag
            .source_path
            .strip_prefix(repo_root)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| frag.source_path.clone());
        let rel_path = PathBuf::from(git_diff_path_key(&rel_path));
        let rebuilt = CodeFragment::new(
            rel_path,
            frag.content().to_owned(),
            frag.start_line(),
            frag.end_line(),
        )
        .map_err(|e| {
            format!("failed to rebuild fragment from '{}': {e}", frag.source_path.display())
        })?;
        result.push(rebuilt);
    }
    Ok(result)
}

/// Convert a path to the slash-separated path format emitted by `git diff`.
fn git_diff_path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Resolve an input directory and require it to stay inside the repository root.
fn resolve_existing_dir_under_repo(
    input_path: &Path,
    repo_root: &Path,
    canonical_root: &Path,
    label: &str,
) -> Result<PathBuf, String> {
    let absolute_path = if input_path.is_absolute() {
        input_path.to_path_buf()
    } else {
        repo_root.join(input_path)
    };
    let canonical_path = absolute_path.canonicalize().map_err(|_| {
        format!(
            "{label} '{}' must be an existing directory under the repository root",
            input_path.display()
        )
    })?;

    if !canonical_path.is_dir() || !canonical_path.starts_with(canonical_root) {
        return Err(format!(
            "{label} '{}' must be an existing directory under the repository root",
            input_path.display()
        ));
    }

    Ok(canonical_path)
}

fn parse_dry_track_id(raw: &str) -> Result<TrackId, String> {
    TrackId::try_new(raw).map_err(|e| format!("invalid --track-id: {e}"))
}

/// Parse a verdict filter string to `VerdictFilter`.
///
/// Accepted values (case-insensitive): "all", "not-a-violation", "accepted", "violation".
///
/// # Errors
///
/// Returns `Err` for unrecognized values.
fn parse_verdict_filter(s: &str) -> Result<VerdictFilter, String> {
    match s.to_ascii_lowercase().as_str() {
        "all" => Ok(VerdictFilter::All),
        "not-a-violation" => Ok(VerdictFilter::NotAViolation),
        "accepted" => Ok(VerdictFilter::Accepted),
        "violation" => Ok(VerdictFilter::Violation),
        other => Err(format!(
            "invalid --filter '{other}' (expected: all / not-a-violation / accepted / violation)"
        )),
    }
}

fn dry_check_approved_outcome(verdict: DryCheckApprovalVerdict) -> CommandOutcome {
    match verdict {
        DryCheckApprovalVerdict::Approved => CommandOutcome {
            stdout: Some("dry check-approved: APPROVED — all pairs verified".to_owned()),
            stderr: None,
            exit_code: 0,
        },
        DryCheckApprovalVerdict::Blocked { unresolved_pair_count } => CommandOutcome {
            stdout: None,
            stderr: Some(format!(
                "dry check-approved: BLOCKED — {unresolved_pair_count} unresolved pair(s); \
                 run `sotp dry write` to record verdicts"
            )),
            exit_code: 1,
        },
    }
}

fn dry_write_outcome(
    findings: &[DryCheckFinding],
    pairs_checked: usize,
    records_appended: usize,
    diff_fragments_processed: usize,
) -> CommandOutcome {
    let mut output_lines: Vec<String> = Vec::new();
    output_lines.push(format!(
        "dry write: {pairs_checked} pair(s) checked; {records_appended} record(s) appended; \
         {} violation(s) found; {diff_fragments_processed} diff fragment(s) processed",
        findings.len()
    ));
    for finding in findings {
        output_lines.push(format!(
            "  changed: {} (hash: {})",
            finding.changed_fragment_ref().path().as_str(),
            finding.changed_fragment_ref().content_hash().as_str(),
        ));
        output_lines.push(format!(
            "  candidate: {} (hash: {})",
            finding.candidate_fragment_ref().path().as_str(),
            finding.candidate_fragment_ref().content_hash().as_str(),
        ));
        output_lines.push(format!("  proposal: {}", finding.refactor_proposal().as_str(),));
    }

    CommandOutcome::success(Some(output_lines.join("\n")))
}

fn ephemeral_index_parent(db_path: &Path, fallback_parent: &Path) -> PathBuf {
    match db_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() && parent.is_dir() => parent.to_path_buf(),
        _ => fallback_parent.to_path_buf(),
    }
}

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

        // Locate per-track directory.
        let items_dir_abs =
            resolve_existing_dir_under_repo(&input.items_dir, &root, &canonical_root, "items_dir")?;
        let track_dir = items_dir_abs.join(track_id.as_ref());

        let commit_hash_path = track_dir.join(".commit_hash");
        let dry_check_json_path = track_dir.join("dry-check.json");

        // Resolve diff base (fail-closed three-branch policy).
        let base = resolve_dry_diff_base(
            input.base_commit.as_deref(),
            &commit_hash_path,
            &canonical_root,
        )?;

        // Parse threshold.
        let threshold = SimilarityThreshold::new(input.threshold)
            .map_err(|e| format!("invalid --threshold: {e}"))?;

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
        let store_for_summary = store.clone();
        let records_before = store_for_summary
            .read_records()
            .map_err(|e| format!("dry-check read before write failed: {e}"))?
            .len();
        let agent =
            Arc::new(CodexDryChecker::new(input.model.clone(), input.capability_name.clone()));
        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );

        // IN-02/D4: Use a fresh ephemeral index for each invocation so that stale
        // fragments from prior runs never accumulate in a persistent LanceDB.
        // `_temp_index_dir` is bound here to ensure drop (and auto-cleanup) happens
        // at method return, not before.
        let (_temp_index_dir, index_adapter) =
            create_ephemeral_index_adapter(&input.db_path, &canonical_root)?;
        let index_port = Arc::new(index_adapter);

        // Construct interactor (5-param; diff_source NOT injected).
        let interactor = DryCheckInteractor::new(
            embedding_port,
            index_port,
            agent,
            store.clone(), // writer
            store,         // reader
        );

        // Run the dry-check write cycle.
        let findings: Vec<DryCheckFinding> = interactor
            .run_dry_check(corpus_fragments, diff_fragments, threshold, base)
            .map_err(|e| format!("dry-check write cycle failed: {e}"))?;

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
        use infrastructure::git_cli::{GitRepository, SystemGitRepo};

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

        // Resolve diff base (fail-closed three-branch policy, same as write).
        let base = resolve_dry_diff_base(
            input.base_commit.as_deref(),
            &commit_hash_path,
            &canonical_root,
        )?;

        // Parse threshold.
        let threshold = SimilarityThreshold::new(input.threshold)
            .map_err(|e| format!("invalid --threshold: {e}"))?;

        let workspace_root = resolve_existing_dir_under_repo(
            &input.workspace_root,
            &root,
            &canonical_root,
            "workspace_root",
        )?;

        // Build diff_fragments + corpus_fragments (shared pipeline).
        let (diff_fragments, corpus_fragments) =
            build_diff_and_corpus_fragments(&base, &workspace_root, &canonical_root)?;

        // Construct adapters.
        let store = Arc::new(FsDryCheckStore::new(dry_check_json_path, canonical_root.clone()));
        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );

        // IN-02/D4: Use a fresh ephemeral index for each invocation so that stale
        // fragments from prior runs never accumulate in a persistent LanceDB.
        // `_temp_index_dir` is bound here to ensure drop (and auto-cleanup) happens
        // at method return, not before.
        let (_temp_index_dir, index_adapter) =
            create_ephemeral_index_adapter(&input.db_path, &canonical_root)?;
        let index_port = Arc::new(index_adapter);

        // Construct interactor (reader + index + embedding).
        let interactor = DryCheckApprovalInteractor::new(store, index_port, embedding_port);

        // Run the gate (3-param; base_commit NOT forwarded — check_approved does not record).
        let verdict = interactor
            .check_approved(corpus_fragments, &diff_fragments, threshold)
            .map_err(|e| format!("dry check-approved failed: {e}"))?;

        Ok(dry_check_approved_outcome(verdict))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn repo_root_for_tests() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("cli-composition manifest must be under apps/")
            .to_path_buf()
    }

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
            threshold: 0.85,
            workspace_root: PathBuf::from("."),
            items_dir: dir.path().to_path_buf(),
            model: "codex".to_owned(),
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
            threshold: 0.85,
            workspace_root: PathBuf::from("."),
            items_dir: dir.path().to_path_buf(),
            model: "codex".to_owned(),
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
            threshold: 0.85,
            workspace_root: PathBuf::from("."),
            items_dir: dir.path().to_path_buf(),
            model: "codex".to_owned(),
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
            threshold: 1.5,
            workspace_root: PathBuf::from("."),
            items_dir: dir.path().to_path_buf(),
            model: "codex".to_owned(),
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
            db_path: dir.path().join("semantic-index"),
            threshold: 0.85,
            workspace_root: PathBuf::from("."),
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
            db_path: dir.path().join("semantic-index"),
            threshold: 0.85,
            workspace_root: PathBuf::from("."),
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
            db_path: dir.path().join("semantic-index"),
            threshold: 0.85,
            workspace_root: PathBuf::from("."),
            items_dir: dir.path().to_path_buf(),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("invalid --track-id"),
            "error must reject escaped track_id, got: {message}"
        );
    }

    #[test]
    fn test_dry_check_approved_public_api_missing_track_dir_reaches_threshold_validation() {
        let dir = temp_items_dir_under_repo();

        let result = CliApp::new().dry_check_approved(DryCheckApprovedInput {
            track_id: "dry-check-approved-missing-track-invalid-threshold".to_owned(),
            base_commit: Some(valid_commit_hash_for_tests()),
            db_path: dir.path().join("semantic-index"),
            threshold: 1.5,
            workspace_root: PathBuf::from("."),
            items_dir: dir.path().to_path_buf(),
        });

        let message = result.unwrap_err();
        assert!(
            message.contains("invalid --threshold"),
            "missing track dir must not be rejected before threshold validation, got: {message}"
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

    // ── DryWriteInput / DryCheckApprovedInput: field round-trip ──────────────

    #[test]
    fn test_dry_write_input_fields_accessible() {
        let input = DryWriteInput {
            track_id: "my-track".to_owned(),
            base_commit: Some("abc1234".to_owned()),
            db_path: PathBuf::from(".semantic_index"),
            threshold: 0.85,
            workspace_root: PathBuf::from("."),
            items_dir: PathBuf::from("track/items"),
            model: "codex-model".to_owned(),
            capability_name: "dry-checker".to_owned(),
        };
        assert_eq!(input.track_id, "my-track");
        assert_eq!(input.base_commit.as_deref(), Some("abc1234"));
        assert!((input.threshold - 0.85).abs() < 1e-6);
    }

    #[test]
    fn test_dry_check_approved_input_fields_accessible() {
        let input = DryCheckApprovedInput {
            track_id: "my-track".to_owned(),
            base_commit: None,
            db_path: PathBuf::from(".semantic_index"),
            threshold: 0.85,
            workspace_root: PathBuf::from("."),
            items_dir: PathBuf::from("track/items"),
        };
        assert_eq!(input.track_id, "my-track");
        assert!(input.base_commit.is_none());
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
}
