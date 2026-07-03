//! Diff/corpus fragment pipeline for `sotp dry write`.
//!
//! Relocated from `apps/cli-composition/src/dry/shared.rs`'s
//! `build_diff_and_corpus_fragments` / `normalize_fragment_paths` /
//! `git_diff_path_key` (T028) — the logic is fully owned by
//! [`super::FsDryCorpusFragmentsAdapter`] now, so it no longer needs to live
//! in `cli_composition`.

use std::path::{Path, PathBuf};

use domain::CommitHash;
use domain::dry_check::{DryCheckCorpusFingerprint, fragments_overlapping_hunks};
use domain::semantic_dup::CodeFragment;
use usecase::dry_check::{DryCheckDiffSource as _, fragment_ref_of};

use crate::dry_check::GitDryCheckDiffGetter;
use crate::dry_check::corpus::sha256_hex;
use crate::semantic_dup::extractor::extract_code_fragments;

/// Convert a path to the slash-separated path format emitted by `git diff`.
pub(super) fn git_diff_path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Normalize a list of `CodeFragment` values so that each `source_path` is
/// repo-relative (the `repo_root` prefix stripped).
///
/// # Errors
///
/// Returns `Err` only when `CodeFragment::new` rejects a rebuilt fragment,
/// which should never happen in practice because the original fragment was
/// already valid.
pub(super) fn normalize_fragment_paths(
    fragments: Vec<CodeFragment>,
    repo_root: &Path,
) -> Result<Vec<CodeFragment>, String> {
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
            format!("failed to rebuild fragment from '{}': {e}", frag.source_path.display())
        })?;
        result.push(rebuilt);
    }
    Ok(result)
}

/// Distinguishes which pipeline stage failed inside
/// [`build_diff_and_corpus_fragments`], so the caller
/// ([`super::FsDryCorpusFragmentsAdapter::build`]) can map each stage to its
/// own `usecase::dry_write_driver::DryCorpusFragmentsError` variant.
pub(super) enum FragmentsBuildError {
    /// `GitDryCheckDiffGetter::list_changed_hunks` failed.
    DiffHunkListing(String),
    /// `extract_code_fragments` failed over `workspace_root`.
    FragmentExtraction(String),
    /// The subsequent `CodeFragment` path-rebuild step failed.
    FragmentPathNormalization(String),
}

/// Build `diff_fragments` and `corpus_fragments` using the hunk-scope pipeline.
///
/// Returns `(diff_fragments, corpus_fragments)` where:
/// - `diff_fragments` are hunk-scoped.
/// - `corpus_fragments` are extracted from `workspace_root` with normalized paths.
///
/// # Errors
///
/// Returns `Err` when diff listing, fragment extraction, or fragment path
/// normalization fails.
pub(super) fn build_diff_and_corpus_fragments(
    base: &CommitHash,
    workspace_root: &Path,
    repo_root: &Path,
) -> Result<(Vec<CodeFragment>, Vec<CodeFragment>), FragmentsBuildError> {
    let getter = GitDryCheckDiffGetter;
    let changed_hunks = getter
        .list_changed_hunks(base, repo_root)
        .map_err(|e| FragmentsBuildError::DiffHunkListing(e.to_string()))?;

    let raw_fragments = extract_code_fragments(workspace_root)
        .map_err(|e| FragmentsBuildError::FragmentExtraction(e.to_string()))?;

    let normalized_fragments = normalize_fragment_paths(raw_fragments, repo_root)
        .map_err(FragmentsBuildError::FragmentPathNormalization)?;

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

fn append_len_prefixed_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

/// Compute the corpus fingerprint over the sorted `(path, content_hash)` pairs
/// of `corpus_fragments`.
///
/// Mirrors `apps/cli-composition/src/dry/corpus_root.rs::compute_dry_corpus_fingerprint_from_fragments`.
pub(super) fn compute_dry_corpus_fingerprint_from_fragments(
    corpus_fragments: &[CodeFragment],
) -> DryCheckCorpusFingerprint {
    let mut entries = Vec::with_capacity(corpus_fragments.len());
    for fragment in corpus_fragments {
        let fragment_ref = match fragment_ref_of(fragment) {
            Ok(fragment_ref) => fragment_ref,
            Err(_) => return DryCheckCorpusFingerprint::fail_closed(),
        };
        entries.push((
            fragment_ref.path().as_str().to_owned(),
            fragment_ref.content_hash().as_str().to_owned(),
        ));
    }
    entries.sort();

    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"sotohe-dry-corpus-fragment-fingerprint-v1\0");
    for (path, content_hash) in entries {
        append_len_prefixed_bytes(&mut canonical, path.as_bytes());
        append_len_prefixed_bytes(&mut canonical, content_hash.as_bytes());
    }

    DryCheckCorpusFingerprint::new(sha256_hex(&canonical))
        .unwrap_or_else(|_| DryCheckCorpusFingerprint::fail_closed())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_git_diff_path_key_windows_separators_returns_slash_path() {
        let path = PathBuf::from(r"src\lib.rs");
        assert_eq!(git_diff_path_key(&path), "src/lib.rs");
    }

    #[test]
    fn test_normalize_fragment_paths_strips_absolute_repo_root() {
        let repo_root = PathBuf::from("/absolute/workspace/root");
        let absolute_path = repo_root.join("src/lib.rs");
        let frag = CodeFragment::new(absolute_path, "fn foo() {}".to_owned(), 1, 1).unwrap();

        let normalized = normalize_fragment_paths(vec![frag], &repo_root).unwrap();

        assert_eq!(normalized.len(), 1);
        assert!(normalized[0].source_path.is_relative());
        assert_eq!(normalized[0].source_path.to_string_lossy(), "src/lib.rs");
    }

    #[test]
    fn test_normalize_fragment_paths_subdir_workspace_returns_repo_relative_path() {
        let repo_root = PathBuf::from("/repo");
        let fragment_path = repo_root.join("apps/cli/src/main.rs");
        let frag = CodeFragment::new(fragment_path, "fn main() {}".to_owned(), 1, 1).unwrap();

        let normalized = normalize_fragment_paths(vec![frag], &repo_root).unwrap();

        assert_eq!(normalized[0].source_path.to_string_lossy(), "apps/cli/src/main.rs");
    }

    #[test]
    fn test_normalize_fragment_paths_keeps_non_prefixed_paths() {
        let repo_root = PathBuf::from("/repo");
        let frag =
            CodeFragment::new(PathBuf::from("other/src/lib.rs"), "fn f() {}".to_owned(), 1, 1)
                .unwrap();

        let normalized = normalize_fragment_paths(vec![frag], &repo_root).unwrap();

        assert_eq!(normalized[0].source_path.to_string_lossy(), "other/src/lib.rs");
    }

    #[test]
    fn test_build_diff_and_corpus_fragments_corpus_is_whole_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("lib.rs"),
            "fn foo() { let x = 1; }\nfn bar() { let y = 2; }\n",
        )
        .unwrap();

        let corpus = extract_code_fragments(dir.path()).expect("extract must succeed");

        assert!(!corpus.is_empty(), "corpus_fragments must not be empty for a non-empty workspace");
    }

    #[test]
    fn test_compute_dry_corpus_fingerprint_from_fragments_ignores_line_span_shift() {
        let path = PathBuf::from("src/lib.rs");
        let content = "pub fn stable() {}\n".to_owned();
        let before = CodeFragment::new(path.clone(), content.clone(), 1, 1).unwrap();
        let after = CodeFragment::new(path, content, 20, 20).unwrap();

        let before_fingerprint = compute_dry_corpus_fingerprint_from_fragments(&[before]);
        let after_fingerprint = compute_dry_corpus_fingerprint_from_fragments(&[after]);

        assert_eq!(before_fingerprint, after_fingerprint);
    }

    #[test]
    fn test_compute_dry_corpus_fingerprint_from_fragments_detects_content_drift() {
        let frag_a =
            CodeFragment::new(PathBuf::from("src/lib.rs"), "pub fn original() {}".to_owned(), 1, 1)
                .unwrap();
        let frag_b =
            CodeFragment::new(PathBuf::from("src/lib.rs"), "pub fn changed() {}".to_owned(), 1, 1)
                .unwrap();

        assert_ne!(
            compute_dry_corpus_fingerprint_from_fragments(&[frag_a]),
            compute_dry_corpus_fingerprint_from_fragments(&[frag_b]),
        );
    }
}
