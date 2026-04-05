use std::collections::BTreeSet;

use domain::CommitHash;
use domain::review_v2::FilePath;
use usecase::review_v2::{DiffGetError, DiffGetter};

use crate::git_cli::{GitRepository, SystemGitRepo};

/// Git-based diff getter that computes the union of 4 diff sources.
///
/// Ported from v1 `GitDiffScopeProvider::changed_files`:
/// 1. `git diff --name-only --diff-filter=ACDMRT $(git merge-base HEAD <base>) HEAD`
/// 2. `git diff --name-only --cached` (staged)
/// 3. `git diff --name-only` (unstaged worktree)
/// 4. `git ls-files --others --exclude-standard` (untracked)
///
/// Deduplicates via `BTreeSet`. Each path is validated through `FilePath::new`.
pub struct GitDiffGetter;

impl DiffGetter for GitDiffGetter {
    fn list_diff_files(&self, base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError> {
        let git = SystemGitRepo::discover()
            .map_err(|e| DiffGetError::Failed(format!("git discover: {e}")))?;

        // 1. Find merge-base between HEAD and base commit
        let merge_base_output = git
            .output(&["merge-base", "HEAD", base.as_ref()])
            .map_err(|e| DiffGetError::Failed(format!("merge-base: {e}")))?;

        if !merge_base_output.status.success() {
            return Err(DiffGetError::Failed(format!(
                "merge-base failed for base {}",
                base.as_ref()
            )));
        }

        let merge_base = String::from_utf8_lossy(&merge_base_output.stdout).trim().to_owned();

        let mut paths = BTreeSet::new();

        // Helper: collect valid FilePaths from git output lines
        let collect = |output: std::process::Output,
                       label: &str,
                       set: &mut BTreeSet<FilePath>|
         -> Result<(), DiffGetError> {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                return Err(DiffGetError::Failed(format!(
                    "{label} failed (exit {}): {stderr}",
                    output.status.code().unwrap_or(-1)
                )));
            }
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                // Only strip trailing \n/\r (git output), NOT leading/trailing spaces
                // (filenames with spaces are valid repo-relative paths)
                let stripped = line.trim_end_matches(['\n', '\r']);
                if stripped.is_empty() {
                    continue;
                }
                // Normalize: replace backslash, strip leading ./
                let normalized = stripped.replace('\\', "/");
                let normalized = normalized.strip_prefix("./").unwrap_or(&normalized);
                if let Ok(fp) = FilePath::new(normalized) {
                    set.insert(fp);
                }
                // Silently drop paths that fail FilePath validation (e.g. traversal)
            }
            Ok(())
        };

        // 2. Committed diff from merge-base to HEAD
        let diff_output = git
            .output(&["diff", "--name-only", "--diff-filter=ACDMRT", &merge_base, "HEAD"])
            .map_err(|e| DiffGetError::Failed(format!("diff: {e}")))?;
        collect(diff_output, "diff merge-base..HEAD", &mut paths)?;

        // 3. Staged but uncommitted
        let staged_output = git
            .output(&["diff", "--name-only", "--cached"])
            .map_err(|e| DiffGetError::Failed(format!("staged diff: {e}")))?;
        collect(staged_output, "diff --cached", &mut paths)?;

        // 4. Unstaged worktree modifications
        let worktree_output = git
            .output(&["diff", "--name-only"])
            .map_err(|e| DiffGetError::Failed(format!("worktree diff: {e}")))?;
        collect(worktree_output, "diff (worktree)", &mut paths)?;

        // 5. Untracked (non-ignored) files
        let untracked_output = git
            .output(&["ls-files", "--others", "--exclude-standard"])
            .map_err(|e| DiffGetError::Failed(format!("ls-files: {e}")))?;
        collect(untracked_output, "ls-files --others", &mut paths)?;

        Ok(paths.into_iter().collect())
    }
}
