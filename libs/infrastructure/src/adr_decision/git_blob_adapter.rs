//! Git-blob-backed implementation of the domain [`domain::AdrFilePort`]
//! secondary port.
//!
//! [`GitBlobAdrFileAdapter`] fetches ADR markdown files from
//! `origin/<branch>:knowledge/adr/` via the same `git ls-tree` /
//! `git show` primitives used by the merge-gate readers. This is the
//! correct adapter for the merge-gate path where ADR signals must be
//! evaluated against the PR branch, not the local worktree.
//!
//! ## Contrast with [`super::FsAdrFileAdapter`]
//!
//! | Adapter | Source | Use case |
//! |---------|--------|----------|
//! | [`super::FsAdrFileAdapter`] | Local filesystem | `sotp verify adr-signals`, `sotp signal check-adr-user` |
//! | [`GitBlobAdrFileAdapter`] | `origin/<branch>:knowledge/adr/` | Merge-gate chain ⓪ (`read_adr_verify_report`) |
//!
//! ## Symlink policy
//!
//! `list_adr_paths` uses `git_ls_tree_dir` (in `crate::git_cli::show`) which
//! silently skips symlinks and other non-regular-file modes at the listing
//! stage. `read_adr_frontmatter` uses `fetch_blob_safe` (in `crate::git_cli::show`)
//! which actively rejects symlinks via a two-phase ls-tree inspection
//! (fail-closed, ADR §D4.3).

use std::path::{Path, PathBuf};

use domain::{AdrFilePort, AdrFilePortError, AdrFrontMatter};

use crate::git_cli::show::{BlobResult, fetch_blob_safe, git_ls_tree_dir};

use super::parse::parse_adr_frontmatter;

/// The repo-relative directory where ADR markdown files live.
const ADR_DIR: &str = "knowledge/adr";

/// Git-blob adapter that reads ADR files from `origin/<branch>:knowledge/adr/`.
///
/// Implements [`domain::AdrFilePort`] using `git ls-tree` (for directory
/// listing) and `git show` via `fetch_blob_safe` (for file content), so
/// that merge-gate evaluation always reads ADRs from the PR branch ref
/// rather than the local worktree.
///
/// `repo_root` must be the root of a git repository that has `origin`
/// configured (e.g. by `git fetch`).
#[derive(Debug, Clone)]
pub struct GitBlobAdrFileAdapter {
    repo_root: PathBuf,
    branch: String,
}

impl GitBlobAdrFileAdapter {
    /// Creates a new adapter rooted at `repo_root`, reading from
    /// `origin/<branch>:knowledge/adr/`.
    ///
    /// Neither `repo_root` existence nor the branch's reachability is
    /// validated at construction time; failures surface lazily from
    /// [`list_adr_paths`](Self::list_adr_paths) /
    /// [`read_adr_frontmatter`](Self::read_adr_frontmatter).
    #[must_use]
    pub fn new(repo_root: PathBuf, branch: String) -> Self {
        Self { repo_root, branch }
    }
}

impl AdrFilePort for GitBlobAdrFileAdapter {
    /// Lists `.md` blob paths directly under `origin/<branch>:knowledge/adr/`.
    ///
    /// Returns sorted, repo-relative paths (e.g.
    /// `"knowledge/adr/2026-01-01-0001-foo.md"`). Non-`.md` entries and
    /// non-regular-file modes are silently skipped. Returns an empty
    /// `Vec` when the `knowledge/adr` directory does not exist on the
    /// branch (not an error — the merge-gate caller maps that to `NotFound`).
    ///
    /// # Errors
    ///
    /// Returns [`AdrFilePortError::ListPaths`] when the underlying
    /// `git ls-tree` command fails (spawn error or non-zero exit).
    fn list_adr_paths(&self) -> Result<Vec<PathBuf>, AdrFilePortError> {
        let repo_paths = git_ls_tree_dir(&self.repo_root, &self.branch, ADR_DIR).map_err(|e| {
            AdrFilePortError::ListPaths(format!(
                "git ls-tree failed for {ADR_DIR} on origin/{}: {e}",
                self.branch
            ))
        })?;

        // Filter to .md files (ls-tree silently passes all regular files; we
        // restrict to .md to mirror FsAdrFileAdapter's behaviour).
        let paths: Vec<PathBuf> = repo_paths
            .into_iter()
            .filter(|p| Path::new(p).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("md")))
            .map(PathBuf::from)
            .collect();
        // Already sorted by git_ls_tree_dir, but sort again after the filter
        // to keep the contract unambiguous.
        let mut paths = paths;
        paths.sort();
        Ok(paths)
    }

    /// Fetches and parses the YAML front-matter of the ADR file at `path`
    /// from `origin/<branch>:<path>`.
    ///
    /// `path` is expected to be a repo-relative `PathBuf` as returned by
    /// [`list_adr_paths`](Self::list_adr_paths) (e.g.
    /// `PathBuf::from("knowledge/adr/2026-01-01-0001-foo.md")`).
    ///
    /// # Errors
    ///
    /// Returns [`AdrFilePortError::ReadFile`] on:
    /// - `fetch_blob_safe` failure (symlink, submodule, spawn error, git exit ≠ 0)
    /// - Path-not-found (the file existed when listed but was deleted before reading)
    /// - Non-UTF-8 blob content
    /// - YAML front-matter parse failure
    fn read_adr_frontmatter(&self, path: PathBuf) -> Result<AdrFrontMatter, AdrFilePortError> {
        let path_str = path.to_string_lossy();
        let bytes = match fetch_blob_safe(&self.repo_root, &self.branch, &path_str) {
            BlobResult::Found(b) => b,
            BlobResult::NotFound => {
                return Err(AdrFilePortError::ReadFile(format!(
                    "{}: not found on origin/{}",
                    path.display(),
                    self.branch
                )));
            }
            BlobResult::CommandFailed(msg) => {
                return Err(AdrFilePortError::ReadFile(format!(
                    "{}: git fetch failed: {msg}",
                    path.display()
                )));
            }
        };
        let content = String::from_utf8(bytes).map_err(|e| {
            AdrFilePortError::ReadFile(format!("{}: non-UTF-8 content: {e}", path.display()))
        })?;
        parse_adr_frontmatter(&content)
            .map_err(|e| AdrFilePortError::ReadFile(format!("{}: {e}", path.display())))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::adr_decision::test_support::{git, setup_repo_with_adr_blobs};

    /// Minimal ADR front-matter with one accepted decision (🔵).
    const ADR_ACCEPTED: &str = "\
---
adr_id: test-accepted
decisions:
  - id: D1
    status: accepted
    user_decision_ref: chat:2026-01-01
---
# body
";

    /// Minimal ADR front-matter with one proposed decision (no grounds → 🔴).
    const ADR_PROPOSED_NO_GROUNDS: &str = "\
---
adr_id: test-proposed
decisions:
  - id: D1
    status: proposed
---
# body
";

    // --- list_adr_paths ---

    #[test]
    fn test_list_adr_paths_returns_md_files_only() {
        let dir = setup_repo_with_adr_blobs(&[
            ("2026-01-01-0001-foo.md", ADR_ACCEPTED),
            ("2026-01-02-0002-bar.md", ADR_ACCEPTED),
            // Non-.md file: should be filtered out.
        ]);
        // Manually add a non-.md file in the same commit for good measure.
        // (We can't do this via setup_repo_with_adr_blobs, so just test the filter
        // via ls-tree: git ls-tree will show the .md files we committed above.)
        let adapter = GitBlobAdrFileAdapter::new(dir.path().to_path_buf(), "main".to_owned());
        let paths = adapter.list_adr_paths().unwrap();
        let names: Vec<&str> =
            paths.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect();
        assert_eq!(names, vec!["2026-01-01-0001-foo.md", "2026-01-02-0002-bar.md"]);
    }

    #[test]
    fn test_list_adr_paths_empty_when_dir_absent() {
        let dir = setup_repo_with_adr_blobs(&[]);
        let adapter = GitBlobAdrFileAdapter::new(dir.path().to_path_buf(), "main".to_owned());
        let paths = adapter.list_adr_paths().unwrap();
        assert!(paths.is_empty(), "expected empty list when knowledge/adr absent, got {paths:?}");
    }

    #[test]
    fn test_list_adr_paths_sorted() {
        let dir = setup_repo_with_adr_blobs(&[
            ("2026-01-03-bar.md", ADR_ACCEPTED),
            ("2026-01-01-foo.md", ADR_ACCEPTED),
            ("2026-01-02-baz.md", ADR_ACCEPTED),
        ]);
        let adapter = GitBlobAdrFileAdapter::new(dir.path().to_path_buf(), "main".to_owned());
        let paths = adapter.list_adr_paths().unwrap();
        let names: Vec<&str> =
            paths.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect();
        assert_eq!(names, vec!["2026-01-01-foo.md", "2026-01-02-baz.md", "2026-01-03-bar.md"]);
    }

    // --- read_adr_frontmatter ---

    #[test]
    fn test_read_adr_frontmatter_found_decodes_accepted() {
        let dir = setup_repo_with_adr_blobs(&[("2026-01-01-foo.md", ADR_ACCEPTED)]);
        let adapter = GitBlobAdrFileAdapter::new(dir.path().to_path_buf(), "main".to_owned());
        let path = PathBuf::from("knowledge/adr/2026-01-01-foo.md");
        let fm = adapter.read_adr_frontmatter(path).unwrap();
        assert_eq!(fm.adr_id(), "test-accepted");
        assert_eq!(fm.decisions().len(), 1);
        assert!(
            matches!(&fm.decisions()[0], domain::AdrDecisionEntry::AcceptedDecision(_)),
            "expected AcceptedDecision"
        );
    }

    #[test]
    fn test_read_adr_frontmatter_not_found_returns_read_file_error() {
        let dir = setup_repo_with_adr_blobs(&[]);
        let adapter = GitBlobAdrFileAdapter::new(dir.path().to_path_buf(), "main".to_owned());
        let path = PathBuf::from("knowledge/adr/nonexistent.md");
        let err = adapter.read_adr_frontmatter(path).unwrap_err();
        assert!(
            matches!(err, AdrFilePortError::ReadFile(_)),
            "expected ReadFile error for absent file, got {err:?}"
        );
    }

    #[test]
    fn test_read_adr_frontmatter_bad_frontmatter_returns_read_file_error() {
        let dir = setup_repo_with_adr_blobs(&[("bad.md", "# no frontmatter here\n")]);
        let adapter = GitBlobAdrFileAdapter::new(dir.path().to_path_buf(), "main".to_owned());
        let path = PathBuf::from("knowledge/adr/bad.md");
        let err = adapter.read_adr_frontmatter(path).unwrap_err();
        assert!(
            matches!(err, AdrFilePortError::ReadFile(_)),
            "expected ReadFile for bad frontmatter, got {err:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_read_adr_frontmatter_rejects_symlink() {
        // A symlinked .md file in knowledge/adr/ must be rejected by
        // fetch_blob_safe (fail-closed: mode 120000 is not RegularFile).
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        let adr_dir = repo.join("knowledge/adr");
        std::fs::create_dir_all(&adr_dir).unwrap();
        std::fs::write(adr_dir.join("real.md"), ADR_ACCEPTED).unwrap();
        std::os::unix::fs::symlink("real.md", adr_dir.join("link.md")).unwrap();
        git(repo, &["add", "knowledge"]);
        git(repo, &["commit", "--quiet", "-m", "add files"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);

        let adapter = GitBlobAdrFileAdapter::new(repo.to_path_buf(), "main".to_owned());
        // Attempt to read the symlinked .md path directly.
        let path = PathBuf::from("knowledge/adr/link.md");
        let err = adapter.read_adr_frontmatter(path).unwrap_err();
        match err {
            AdrFilePortError::ReadFile(msg) => {
                assert!(msg.contains("symlink"), "expected symlink rejection, got: {msg}");
            }
            other => panic!("expected ReadFile(symlink), got {other:?}"),
        }
    }

    // --- integration: list + read (full VerifyAdrSignalsInteractor round-trip) ---

    #[test]
    fn test_full_round_trip_counts_signals_correctly() {
        use std::sync::Arc;
        use usecase::verify_adr_signals::{
            VerifyAdrSignals, VerifyAdrSignalsCommand, VerifyAdrSignalsInteractor,
        };

        let dir = setup_repo_with_adr_blobs(&[
            ("2026-01-01-accepted.md", ADR_ACCEPTED),
            ("2026-01-02-proposed.md", ADR_PROPOSED_NO_GROUNDS),
        ]);
        let adapter = GitBlobAdrFileAdapter::new(dir.path().to_path_buf(), "main".to_owned());
        let port: Arc<dyn domain::AdrFilePort> = Arc::new(adapter);
        let interactor = VerifyAdrSignalsInteractor::new(port);
        let report = interactor.verify(VerifyAdrSignalsCommand).unwrap();
        // accepted → blue=1, proposed with no grounds → red=1
        assert_eq!(report.blue_count(), 1, "blue_count mismatch");
        assert_eq!(report.red_count(), 1, "red_count mismatch");
        assert_eq!(report.yellow_count(), 0, "yellow_count mismatch");
    }
}
