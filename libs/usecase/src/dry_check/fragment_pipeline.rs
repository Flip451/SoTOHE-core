//! D4 diff-fragment pipeline extraction — application service + interactor.
//!
//! Extracts `build_current_fragment_refs` from `cli_composition` into a
//! usecase application service. The pipeline:
//! 1. Lists changed git hunks via [`DryCheckDiffSource`] (CWD rooted at
//!    `repo_root` by the caller before invoking this interactor).
//! 2. Extracts all Rust code fragments from `canonical_root`.
//! 3. Normalises fragment paths to repo-relative form.
//! 4. Filters fragments to changed paths only.
//! 5. Narrows to hunk-overlapping fragments via
//!    [`domain::dry_check::fragments_overlapping_hunks`].
//! 6. Derives a [`FragmentRef`] per fragment via [`fragment_ref_of`].
//!
//! The CWD guard (needed by `GitDryCheckDiffGetter::list_changed_hunks` for
//! `SystemGitRepo::discover()`) is managed by the **caller** in
//! `cli_composition`: the caller sets CWD to `repo_root`, then delegates to
//! `DryFragmentPipelineService::derive_current_refs`, then restores CWD. This
//! keeps the usecase layer free of `std::env::set_current_dir`.
//!
//! ADR: `knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md` D4.
//! Spec: AC-07.

use std::collections::BTreeSet;
use std::path::PathBuf;

use domain::CommitHash;
use domain::dry_check::{FragmentRef, fragments_overlapping_hunks};
use domain::semantic_dup::CodeFragment;

use crate::d4_orchestration::D4OrchestrationError;
use crate::dry_check::ports::DryCheckDiffSource;
use crate::dry_check::shared::fragment_ref_of;

// ── CodeFragmentExtractorPort ─────────────────────────────────────────────────

/// Secondary port for extracting code fragments from a workspace root.
///
/// Abstracts `infrastructure::semantic_dup::extractor::extract_code_fragments`
/// so the usecase interactor does not import from `infrastructure` directly.
pub trait CodeFragmentExtractorPort: Send + Sync {
    /// Extract all code fragments under `workspace_root`.
    ///
    /// # Errors
    ///
    /// Returns a `String` error description on I/O or parse failure.
    fn extract(&self, workspace_root: &std::path::Path) -> Result<Vec<CodeFragment>, String>;
}

// ── DryFragmentPipelineCommand ────────────────────────────────────────────────

/// CQRS command for the D4 diff-fragment and dry-check fragment-ref derivation
/// pipeline.
///
/// `canonical_root` is the project root (parent of `track/items`); it is
/// passed to the fragment extractor so that all Rust source files in the
/// project are scanned.
///
/// `repo_root` is the git repository root (returned by `SystemGitRepo::root()`);
/// it is used for path normalization so that fragment source paths match the
/// repo-relative paths emitted by `GitDryCheckDiffGetter`.
///
/// In the common case `canonical_root == repo_root`; they differ only when the
/// project lives in a subdirectory of a monorepo.
///
/// **CWD contract**: the caller (`cli_composition`) must set `CWD = repo_root`
/// before calling `derive_current_refs` and restore it afterwards. This keeps
/// `std::env::set_current_dir` out of the usecase layer.
#[derive(Debug, Clone)]
pub struct DryFragmentPipelineCommand {
    /// Project root (parent of `track/items`); used as the fragment extraction
    /// workspace root.
    pub canonical_root: PathBuf,
    /// Git repository root; used for path normalization.
    pub repo_root: PathBuf,
    /// Diff base commit hash.
    pub base: CommitHash,
}

// ── DryFragmentPipelineOutput ─────────────────────────────────────────────────

/// Output DTO for the D4 dry fragment pipeline.
///
/// Carries the derived `FragmentRef` set for the current diff scope.
/// The `records_*` counters are always `0` in this service — they were part of
/// the original type catalogue sketch but the record-count aggregation lives
/// in the dry-write path (which is not extracted here). Callers can ignore
/// them; they remain in the struct for catalogue compatibility.
#[derive(Debug, Clone)]
pub struct DryFragmentPipelineOutput {
    /// Derived fragment refs for all diff-scoped fragments.
    pub fragment_refs: BTreeSet<FragmentRef>,
    /// Number of dry-check records before the write cycle (always 0 here).
    pub records_before: u32,
    /// Number of dry-check records after the write cycle (always 0 here).
    pub records_after: u32,
    /// Number of dry-check records appended in the write cycle (always 0 here).
    pub records_appended: u32,
}

// ── DryFragmentPipelineService ────────────────────────────────────────────────

/// Application service (primary port) for the D4 diff-fragment pipeline.
///
/// `Send + Sync` required for `dyn DryFragmentPipelineService`.
pub trait DryFragmentPipelineService: Send + Sync {
    /// Run the diff-fragment pipeline and return the derived `FragmentRef` set.
    ///
    /// # Errors
    ///
    /// Returns [`D4OrchestrationError::DiffFragment`] when hunk listing,
    /// fragment extraction, path normalization, or ref derivation fails.
    fn derive_current_refs(
        &self,
        cmd: DryFragmentPipelineCommand,
    ) -> Result<DryFragmentPipelineOutput, D4OrchestrationError>;
}

// ── DryFragmentPipelineInteractor ─────────────────────────────────────────────

/// Interactor implementing [`DryFragmentPipelineService`].
///
/// Holds an injected [`DryCheckDiffSource`] port (for hunk listing) and a
/// [`CodeFragmentExtractorPort`] (for source-file extraction). The CWD guard
/// (needed by `GitDryCheckDiffGetter`) is managed by the caller in
/// `cli_composition`.
pub struct DryFragmentPipelineInteractor {
    diff_source: std::sync::Arc<dyn DryCheckDiffSource>,
    fragment_extractor: std::sync::Arc<dyn CodeFragmentExtractorPort>,
}

impl DryFragmentPipelineInteractor {
    /// Construct a new interactor with injected ports.
    #[must_use]
    pub fn new(
        diff_source: std::sync::Arc<dyn DryCheckDiffSource>,
        fragment_extractor: std::sync::Arc<dyn CodeFragmentExtractorPort>,
    ) -> Self {
        Self { diff_source, fragment_extractor }
    }
}

impl DryFragmentPipelineService for DryFragmentPipelineInteractor {
    fn derive_current_refs(
        &self,
        cmd: DryFragmentPipelineCommand,
    ) -> Result<DryFragmentPipelineOutput, D4OrchestrationError> {
        let DryFragmentPipelineCommand { canonical_root, repo_root, base } = cmd;

        // 1. List changed hunks. The diff source is rooted explicitly at `repo_root`
        //    via the port signature — no ambient CWD dependency.
        let changed_hunks =
            self.diff_source.list_changed_hunks(&base, &repo_root).map_err(|e| {
                D4OrchestrationError::DiffFragment(format!("list_changed_hunks failed: {e}"))
            })?;

        // 2. Extract all code fragments from the project root.
        let raw_fragments = self.fragment_extractor.extract(&canonical_root).map_err(|e| {
            D4OrchestrationError::DiffFragment(format!("fragment extraction failed: {e}"))
        })?;

        // 3. Normalize fragment paths to repo-relative form.
        let mut normalized: Vec<CodeFragment> = Vec::with_capacity(raw_fragments.len());
        for frag in raw_fragments {
            let rel = frag
                .source_path
                .strip_prefix(&repo_root)
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|_| frag.source_path.clone());
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let rebuilt = CodeFragment::new(
                std::path::PathBuf::from(&rel_str),
                frag.content().to_owned(),
                frag.start_line(),
                frag.end_line(),
            )
            .map_err(|e| {
                D4OrchestrationError::DiffFragment(format!("fragment rebuild failed: {e}"))
            })?;
            normalized.push(rebuilt);
        }

        // 4. Filter to fragments whose path appears in the changed-path set.
        let changed_paths: std::collections::HashSet<String> =
            changed_hunks.iter().map(|h| h.path().as_str().to_owned()).collect();

        let candidates: Vec<CodeFragment> = normalized
            .iter()
            .filter(|f| {
                let key = f.source_path.to_string_lossy().replace('\\', "/");
                changed_paths.contains(key.as_str())
            })
            .cloned()
            .collect();

        // 5. Narrow to hunk-overlapping fragments.
        let diff_fragments = fragments_overlapping_hunks(&candidates, &changed_hunks);

        // 6. Derive a FragmentRef per fragment.
        let mut fragment_refs: BTreeSet<FragmentRef> = BTreeSet::new();
        for fragment in &diff_fragments {
            let r = fragment_ref_of(fragment).map_err(|e| {
                D4OrchestrationError::DiffFragment(format!("fragment ref derivation failed: {e}"))
            })?;
            fragment_refs.insert(r);
        }

        Ok(DryFragmentPipelineOutput {
            fragment_refs,
            records_before: 0,
            records_after: 0,
            records_appended: 0,
        })
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use domain::CommitHash;
    use domain::dry_check::{DiffFileHunks, DiffHunkRange};
    use domain::review_v2::types::FilePath;
    use domain::semantic_dup::CodeFragment;
    use mockall::mock;

    use super::*;
    use crate::dry_check::errors::DryCheckDiffError;

    // ── Mock: DryCheckDiffSource ──────────────────────────────────────────────

    mock! {
        pub MockDiffSource {}
        impl DryCheckDiffSource for MockDiffSource {
            fn list_changed_hunks(
                &self,
                base: &CommitHash,
                repo_root: &std::path::Path,
            ) -> Result<Vec<DiffFileHunks>, DryCheckDiffError>;
        }
    }

    // ── Mock: CodeFragmentExtractorPort ───────────────────────────────────────

    mock! {
        pub MockExtractor {}
        impl CodeFragmentExtractorPort for MockExtractor {
            fn extract(&self, workspace_root: &std::path::Path) -> Result<Vec<CodeFragment>, String>;
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_commit() -> CommitHash {
        CommitHash::try_new("a".repeat(40)).unwrap()
    }

    fn make_fragment(path: &str, content: &str) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), content.to_owned(), 1, 5).unwrap()
    }

    fn make_hunk(path: &str, start: u32, end: u32) -> DiffFileHunks {
        let file_path = FilePath::new(path.to_owned()).unwrap();
        let hunk_range = DiffHunkRange::new(start, end).unwrap();
        DiffFileHunks::new(file_path, vec![hunk_range]).unwrap()
    }

    fn make_interactor(
        diff_source: MockMockDiffSource,
        extractor: MockMockExtractor,
    ) -> DryFragmentPipelineInteractor {
        DryFragmentPipelineInteractor::new(Arc::new(diff_source), Arc::new(extractor))
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    /// Happy path: a changed fragment that overlaps a hunk produces one FragmentRef.
    #[test]
    fn derive_current_refs_returns_fragment_ref_for_changed_fragment() {
        let repo_root = PathBuf::from("/repo");
        let canonical_root = repo_root.clone();
        let base = make_commit();

        // One hunk in src/a.rs lines 1–5.
        let mut diff_source = MockMockDiffSource::new();
        diff_source
            .expect_list_changed_hunks()
            .times(1)
            .returning(|_, _| Ok(vec![make_hunk("src/a.rs", 1, 5)]));

        // One fragment at /repo/src/a.rs that overlaps the hunk.
        let fragment = make_fragment("/repo/src/a.rs", "fn a() {}");
        let mut extractor = MockMockExtractor::new();
        let fragment_clone = fragment.clone();
        extractor.expect_extract().times(1).returning(move |_| Ok(vec![fragment_clone.clone()]));

        let interactor = make_interactor(diff_source, extractor);
        let cmd = DryFragmentPipelineCommand { canonical_root, repo_root, base };
        let output = interactor.derive_current_refs(cmd).unwrap();

        assert_eq!(output.fragment_refs.len(), 1, "expected exactly one FragmentRef");
        // Verify the path was normalized to repo-relative form.
        let ref_path = output.fragment_refs.iter().next().unwrap().path().as_str();
        assert_eq!(ref_path, "src/a.rs", "path must be repo-relative");
        // Metrics counters are always 0 for this interactor.
        assert_eq!(output.records_before, 0);
        assert_eq!(output.records_after, 0);
        assert_eq!(output.records_appended, 0);
    }

    /// Fragments not in the changed-path set are excluded from the output.
    #[test]
    fn derive_current_refs_excludes_unchanged_fragments() {
        let repo_root = PathBuf::from("/repo");
        let canonical_root = repo_root.clone();
        let base = make_commit();

        // One hunk in src/a.rs; src/b.rs is NOT changed.
        let mut diff_source = MockMockDiffSource::new();
        diff_source
            .expect_list_changed_hunks()
            .times(1)
            .returning(|_, _| Ok(vec![make_hunk("src/a.rs", 1, 5)]));

        // Two fragments — only src/a.rs is in the diff.
        let frag_a = make_fragment("/repo/src/a.rs", "fn a() {}");
        let frag_b = make_fragment("/repo/src/b.rs", "fn b() {}");
        let mut extractor = MockMockExtractor::new();
        extractor
            .expect_extract()
            .times(1)
            .returning(move |_| Ok(vec![frag_a.clone(), frag_b.clone()]));

        let interactor = make_interactor(diff_source, extractor);
        let cmd = DryFragmentPipelineCommand { canonical_root, repo_root, base };
        let output = interactor.derive_current_refs(cmd).unwrap();

        assert_eq!(output.fragment_refs.len(), 1, "only src/a.rs fragment should be included");
        let ref_path = output.fragment_refs.iter().next().unwrap().path().as_str();
        assert_eq!(ref_path, "src/a.rs");
    }

    /// When no fragments overlap any hunk the output is an empty BTreeSet.
    #[test]
    fn derive_current_refs_returns_empty_set_when_no_hunk_overlap() {
        let repo_root = PathBuf::from("/repo");
        let canonical_root = repo_root.clone();
        let base = make_commit();

        // Hunk covers lines 10–20; fragment covers lines 1–5 — no overlap.
        let mut diff_source = MockMockDiffSource::new();
        diff_source
            .expect_list_changed_hunks()
            .times(1)
            .returning(|_, _| Ok(vec![make_hunk("src/a.rs", 10, 20)]));

        let frag = make_fragment("/repo/src/a.rs", "fn a() {}");
        let mut extractor = MockMockExtractor::new();
        extractor.expect_extract().times(1).returning(move |_| Ok(vec![frag.clone()]));

        let interactor = make_interactor(diff_source, extractor);
        let cmd = DryFragmentPipelineCommand { canonical_root, repo_root, base };
        let output = interactor.derive_current_refs(cmd).unwrap();

        assert!(output.fragment_refs.is_empty(), "no overlap => empty output");
    }

    /// When the diff is empty the output is an empty BTreeSet.
    #[test]
    fn derive_current_refs_returns_empty_set_when_no_changed_hunks() {
        let repo_root = PathBuf::from("/repo");
        let canonical_root = repo_root.clone();
        let base = make_commit();

        let mut diff_source = MockMockDiffSource::new();
        diff_source.expect_list_changed_hunks().times(1).returning(|_, _| Ok(vec![]));

        let mut extractor = MockMockExtractor::new();
        extractor.expect_extract().times(1).returning(|_| Ok(vec![]));

        let interactor = make_interactor(diff_source, extractor);
        let cmd = DryFragmentPipelineCommand { canonical_root, repo_root, base };
        let output = interactor.derive_current_refs(cmd).unwrap();

        assert!(output.fragment_refs.is_empty());
    }

    /// Diff-source error propagates as DiffFragment variant.
    #[test]
    fn derive_current_refs_propagates_diff_source_error() {
        let repo_root = PathBuf::from("/repo");
        let canonical_root = repo_root.clone();
        let base = make_commit();

        let mut diff_source = MockMockDiffSource::new();
        diff_source
            .expect_list_changed_hunks()
            .times(1)
            .returning(|_, _| Err(DryCheckDiffError::Failed("git error".to_owned())));

        let extractor = MockMockExtractor::new();
        let interactor = make_interactor(diff_source, extractor);
        let cmd = DryFragmentPipelineCommand { canonical_root, repo_root, base };

        let err = interactor.derive_current_refs(cmd).unwrap_err();
        assert!(
            matches!(err, D4OrchestrationError::DiffFragment(_)),
            "diff-source error must map to DiffFragment variant"
        );
    }

    /// Fragment-extractor error propagates as DiffFragment variant.
    #[test]
    fn derive_current_refs_propagates_extractor_error() {
        let repo_root = PathBuf::from("/repo");
        let canonical_root = repo_root.clone();
        let base = make_commit();

        let mut diff_source = MockMockDiffSource::new();
        diff_source.expect_list_changed_hunks().times(1).returning(|_, _| Ok(vec![]));

        let mut extractor = MockMockExtractor::new();
        extractor.expect_extract().times(1).returning(|_| Err("extraction failed".to_owned()));

        let interactor = make_interactor(diff_source, extractor);
        let cmd = DryFragmentPipelineCommand { canonical_root, repo_root, base };

        let err = interactor.derive_current_refs(cmd).unwrap_err();
        assert!(
            matches!(err, D4OrchestrationError::DiffFragment(_)),
            "extractor error must map to DiffFragment variant"
        );
    }

    /// Two fragments in the same changed file produce two distinct FragmentRefs
    /// (different content hash).
    #[test]
    fn derive_current_refs_produces_distinct_refs_for_two_fragments_in_same_file() {
        let repo_root = PathBuf::from("/repo");
        let canonical_root = repo_root.clone();
        let base = make_commit();

        // Large hunk covering both fragments.
        let mut diff_source = MockMockDiffSource::new();
        diff_source
            .expect_list_changed_hunks()
            .times(1)
            .returning(|_, _| Ok(vec![make_hunk("src/a.rs", 1, 20)]));

        let frag1 =
            CodeFragment::new(PathBuf::from("/repo/src/a.rs"), "fn a() {}".to_owned(), 1, 5)
                .unwrap();
        let frag2 =
            CodeFragment::new(PathBuf::from("/repo/src/a.rs"), "fn b() {}".to_owned(), 8, 12)
                .unwrap();
        let mut extractor = MockMockExtractor::new();
        extractor
            .expect_extract()
            .times(1)
            .returning(move |_| Ok(vec![frag1.clone(), frag2.clone()]));

        let interactor = make_interactor(diff_source, extractor);
        let cmd = DryFragmentPipelineCommand { canonical_root, repo_root, base };
        let output = interactor.derive_current_refs(cmd).unwrap();

        assert_eq!(output.fragment_refs.len(), 2, "two distinct content hashes => two refs");
    }

    /// Fragments outside repo_root retain their original path (no strip).
    #[test]
    fn derive_current_refs_retains_original_path_when_outside_repo_root() {
        let repo_root = PathBuf::from("/different-repo");
        let canonical_root = PathBuf::from("/repo");
        let base = make_commit();

        // Hunk uses the raw path (as would be emitted by git for this project).
        let mut diff_source = MockMockDiffSource::new();
        diff_source
            .expect_list_changed_hunks()
            .times(1)
            .returning(|_, _| Ok(vec![make_hunk("src/a.rs", 1, 5)]));

        // Fragment lives under /repo, not /different-repo — strip_prefix fails.
        // The path is kept as-is (absolute), so path filter will not match "src/a.rs"
        // => no output.
        let frag = make_fragment("/repo/src/a.rs", "fn a() {}");
        let mut extractor = MockMockExtractor::new();
        extractor.expect_extract().times(1).returning(move |_| Ok(vec![frag.clone()]));

        let interactor = make_interactor(diff_source, extractor);
        let cmd = DryFragmentPipelineCommand { canonical_root, repo_root, base };
        let output = interactor.derive_current_refs(cmd).unwrap();

        // Fragment path "/repo/src/a.rs" (absolute, no strip) won't match hunk "src/a.rs".
        assert!(
            output.fragment_refs.is_empty(),
            "fragment outside repo_root must not match hunk path"
        );
    }
}
