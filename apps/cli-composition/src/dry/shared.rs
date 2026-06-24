use std::path::{Path, PathBuf};

use domain::dry_check::{
    DryCheckApprovalVerdict, DryCheckFinding, VerdictFilter, fragments_overlapping_hunks,
};
use domain::semantic_dup::CodeFragment;
use domain::{CommitHash, TrackId};
use infrastructure::dry_check::{
    DryCheckCommitHashError, FsDryCheckCommitHashStore, GitDryCheckDiffGetter,
};
use infrastructure::semantic_dup::extractor::extract_code_fragments;

use crate::{CommandOutcome, error::CompositionError};

/// Resolve the diff base commit using the three-branch fail-closed policy.
///
/// Branch 1: `FsDryCheckCommitHashStore::read()` -> `Ok(Some(hash))` -> use it.
/// Branch 2: `Ok(None)` (file absent or non-ancestor) -> fall back to
///   `git rev-parse main`.
/// Branch 3: `Err(DryCheckCommitHashError::Format)` -> emit `eprintln!` warn
///   and fall back to `git rev-parse main` (absorbed; must not abort the gate).
///
/// CN-01: uses dry-check's own `FsDryCheckCommitHashStore`, never
/// `review_v2`'s `FsCommitHashStore`.
///
/// When `base_commit_override` is `Some`, the string is parsed to `CommitHash`
/// and returned directly (skips the store lookup entirely).
///
/// # Errors
///
/// Returns `Err` only when `base_commit_override` is invalid, or when
/// `git rev-parse main` fails.
pub(super) fn resolve_dry_diff_base(
    base_commit_override: Option<&str>,
    commit_hash_path: &Path,
    trusted_root: &Path,
) -> Result<CommitHash, CompositionError> {
    if let Some(s) = base_commit_override {
        return CommitHash::try_new(s)
            .map_err(|e| CompositionError::Infrastructure(format!("invalid --base-commit: {e}")));
    }

    resolve_dry_diff_base_from_store(commit_hash_path, trusted_root, None, "dry-check")
}

/// Shared three-branch fail-closed diff-base resolution using the
/// `FsDryCheckCommitHashStore`.
///
/// Branch 1: `FsDryCheckCommitHashStore::read()` -> `Ok(Some(hash))` -> use it.
/// Branch 2: `Ok(None)` (file absent or non-ancestor) -> fall back to
///   `git rev-parse main`.
/// Branch 3: `Err(DryCheckCommitHashError::Format)` / other -> emit `eprintln!`
///   warn and fall back to `git rev-parse main` (absorbed; must not abort the gate).
///
/// `git_discovery_root`: when `Some`, git is discovered from that path via
/// `SystemGitRepo::discover_from`; when `None`, `SystemGitRepo::discover()` is
/// used (CWD-based discovery).
///
/// `warning_prefix`: used in `[warn] <prefix>:` messages (e.g. `"dry-check"`,
/// `"fixpoint-resolve"`).
///
/// # Errors
///
/// Returns `Err` only when `git rev-parse main` fails.
pub(crate) fn resolve_dry_diff_base_from_store(
    commit_hash_path: &Path,
    trusted_root: &Path,
    git_discovery_root: Option<&Path>,
    warning_prefix: &str,
) -> Result<CommitHash, CompositionError> {
    let store =
        FsDryCheckCommitHashStore::new(commit_hash_path.to_path_buf(), trusted_root.to_path_buf());
    match store.read() {
        Ok(Some(hash)) => return Ok(hash),
        Ok(None) => {}
        Err(DryCheckCommitHashError::Format(detail)) => {
            eprintln!(
                "[warn] {warning_prefix}: malformed .commit_hash ({detail}); falling back to main"
            );
        }
        Err(other) => {
            eprintln!(
                "[warn] {warning_prefix}: failed to read .commit_hash ({other}); falling back to main"
            );
        }
    }

    git_rev_parse_main_at(git_discovery_root)
}

/// Run `git rev-parse main` and return the resulting `CommitHash`.
///
/// When `discovery_root` is `Some`, git is discovered from that path; otherwise
/// CWD-based discovery is used.
///
/// # Errors
///
/// Returns `Err` when git cannot be discovered, the command fails, or the
/// output is not a valid commit hash.
pub(crate) fn git_rev_parse_main_at(
    discovery_root: Option<&Path>,
) -> Result<CommitHash, CompositionError> {
    use infrastructure::git_cli::{GitRepository, SystemGitRepo};

    let git = match discovery_root {
        Some(root) => SystemGitRepo::discover_from(root)
            .map_err(|e| CompositionError::Infrastructure(format!("git discover: {e}")))?,
        None => SystemGitRepo::discover()
            .map_err(|e| CompositionError::Infrastructure(format!("git discover: {e}")))?,
    };
    let output = git
        .output(&["rev-parse", "main"])
        .map_err(|e| CompositionError::Infrastructure(format!("git rev-parse main: {e}")))?;
    if !output.status.success() {
        return Err(CompositionError::Infrastructure("git rev-parse main failed".to_owned()));
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    CommitHash::try_new(&sha)
        .map_err(|e| CompositionError::Infrastructure(format!("invalid main SHA: {e}")))
}

/// Build `diff_fragments` using the hunk-scope pipeline.
///
/// Returns `(diff_fragments, corpus_fragments)` where:
/// - `diff_fragments` are hunk-scoped.
/// - `corpus_fragments` are extracted from `workspace_root` with normalized paths.
///
/// # Errors
///
/// Returns `Err` when diff listing or fragment extraction fails.
pub(super) fn build_diff_and_corpus_fragments(
    base: &CommitHash,
    workspace_root: &Path,
    repo_root: &Path,
) -> Result<(Vec<CodeFragment>, Vec<CodeFragment>), CompositionError> {
    use usecase::dry_check::DryCheckDiffSource as _;

    let getter = GitDryCheckDiffGetter;
    let changed_hunks = getter
        .list_changed_hunks(base, repo_root)
        .map_err(|e| CompositionError::Infrastructure(format!("list_changed_hunks failed: {e}")))?;

    let raw_fragments = extract_code_fragments(workspace_root).map_err(|e| {
        CompositionError::Infrastructure(format!("fragment extraction failed: {e}"))
    })?;

    let normalized_fragments = normalize_fragment_paths(raw_fragments, repo_root).map_err(|e| {
        CompositionError::Infrastructure(format!("fragment path normalization failed: {e}"))
    })?;

    let changed_paths: std::collections::HashSet<String> =
        changed_hunks.iter().map(|h| h.path().as_str().to_owned()).collect();

    let candidate_fragments: Vec<CodeFragment> = normalized_fragments
        .iter()
        .filter(|f| {
            let path_key = git_diff_path_key(&f.source_path);
            changed_paths.contains(path_key.as_str())
        })
        .cloned()
        .collect();

    let diff_fragments = fragments_overlapping_hunks(&candidate_fragments, &changed_hunks);
    let corpus_fragments = normalized_fragments;

    Ok((diff_fragments, corpus_fragments))
}

/// Normalize a list of `CodeFragment` values so that each `source_path` is
/// repo-relative (the `repo_root` prefix stripped).
///
/// # Errors
///
/// Returns `Err` only when `CodeFragment::new` rejects a rebuilt fragment, which
/// should never happen in practice because the original fragment was already valid.
pub(super) fn normalize_fragment_paths(
    fragments: Vec<CodeFragment>,
    repo_root: &Path,
) -> Result<Vec<CodeFragment>, CompositionError> {
    let mut result = Vec::with_capacity(fragments.len());
    for frag in fragments {
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
            CompositionError::Infrastructure(format!(
                "failed to rebuild fragment from '{}': {e}",
                frag.source_path.display()
            ))
        })?;
        result.push(rebuilt);
    }
    Ok(result)
}

/// Convert a path to the slash-separated path format emitted by `git diff`.
pub(super) fn git_diff_path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Resolve an input directory and require it to stay inside the repository root.
pub(super) fn resolve_existing_dir_under_repo(
    input_path: &Path,
    repo_root: &Path,
    canonical_root: &Path,
    label: &str,
) -> Result<PathBuf, CompositionError> {
    let absolute_path = if input_path.is_absolute() {
        input_path.to_path_buf()
    } else {
        repo_root.join(input_path)
    };
    let canonical_path = absolute_path.canonicalize().map_err(|_| {
        CompositionError::WiringFailed(format!(
            "{label} '{}' must be an existing directory under the repository root",
            input_path.display()
        ))
    })?;

    if !canonical_path.is_dir() || !canonical_path.starts_with(canonical_root) {
        return Err(CompositionError::WiringFailed(format!(
            "{label} '{}' must be an existing directory under the repository root",
            input_path.display()
        )));
    }

    Ok(canonical_path)
}

pub(super) fn parse_dry_track_id(raw: &str) -> Result<TrackId, CompositionError> {
    TrackId::try_new(raw)
        .map_err(|e| CompositionError::WiringFailed(format!("invalid --track-id: {e}")))
}

/// Parse a verdict filter string to `VerdictFilter`.
///
/// Accepted values (case-insensitive): "all", "not-a-violation", "accepted", "violation".
///
/// # Errors
///
/// Returns `Err` for unrecognized values.
pub(super) fn parse_verdict_filter(s: &str) -> Result<VerdictFilter, CompositionError> {
    match s.to_ascii_lowercase().as_str() {
        "all" => Ok(VerdictFilter::All),
        "not-a-violation" => Ok(VerdictFilter::NotAViolation),
        "accepted" => Ok(VerdictFilter::Accepted),
        "violation" => Ok(VerdictFilter::Violation),
        other => Err(CompositionError::WiringFailed(format!(
            "invalid --filter '{other}' (expected: all / not-a-violation / accepted / violation)"
        ))),
    }
}

pub(super) fn dry_check_approved_outcome(verdict: DryCheckApprovalVerdict) -> CommandOutcome {
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

pub(super) fn dry_write_outcome(
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
