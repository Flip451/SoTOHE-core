use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Output;

use domain::CommitHash;
use domain::review_v2::FilePath;
use usecase::review_v2::{DiffGetError, DiffGetter};

use crate::git_cli::{SystemGitRepo, isolated_bounded_git_output};

/// Review-state diff reads are gate-defining, so keep both stdout and stderr
/// bounded while using the isolated git lane.
const MAX_REVIEW_DIFF_OUTPUT_BYTES: usize = 1024 * 1024;

/// Git-based diff getter that computes the union of 4 diff sources.
///
/// Ported from v1 `GitDiffScopeProvider::changed_files`:
/// 1. `git diff --name-only -z --diff-filter=ACDMRT $(git merge-base HEAD <base>) HEAD`
/// 2. `git diff --name-only -z --cached` (staged)
/// 3. `git diff --name-only -z` (unstaged worktree)
/// 4. `git ls-files --others --exclude-standard -z` (untracked)
///
/// Deduplicates via `BTreeSet`. Each path is validated through `FilePath::new`.
pub struct GitDiffGetter;

impl DiffGetter for GitDiffGetter {
    fn list_diff_files(&self, base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError> {
        let git = SystemGitRepo::discover()
            .map_err(|e| DiffGetError::Failed(format!("git discover: {e}")))?;
        list_diff_files_from_root(git.root(), base)
    }
}

/// Repository-rooted diff getter for callers that have already resolved the
/// repository whose review state is being evaluated.
///
/// This stays crate-private because the public review-v2 composition continues
/// to use [`GitDiffGetter`]. It avoids rediscovering a repository from the
/// process CWD for trusted-root checks.
pub(crate) struct RootedGitDiffGetter {
    root: PathBuf,
}

impl RootedGitDiffGetter {
    pub(crate) fn new(git: SystemGitRepo) -> Self {
        Self { root: git.root().to_path_buf() }
    }
}

impl DiffGetter for RootedGitDiffGetter {
    fn list_diff_files(&self, base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError> {
        list_diff_files_from_root(&self.root, base)
    }
}

/// Runs every review diff query through the isolated bounded Git lane.
///
/// Both the composition-facing getter and the trusted-root state getter use
/// this one function. A review gate therefore cannot select an unbounded
/// `Command::output` path merely because its repository came from process CWD.
fn list_diff_files_from_root(
    root: &std::path::Path,
    base: &CommitHash,
) -> Result<Vec<FilePath>, DiffGetError> {
    list_diff_files_with(base, |args| {
        isolated_bounded_git_output(root, args, MAX_REVIEW_DIFF_OUTPUT_BYTES).map_err(|error| {
            DiffGetError::Failed(format!("isolated git {}: {error}", args.join(" ")))
        })
    })
}

fn list_diff_files_with(
    base: &CommitHash,
    mut run_git: impl FnMut(&[&str]) -> Result<Output, DiffGetError>,
) -> Result<Vec<FilePath>, DiffGetError> {
    // 1. Find merge-base between HEAD and base commit
    let merge_base_output = run_git(&["merge-base", "HEAD", base.as_ref()])?;

    if !merge_base_output.status.success() {
        return Err(DiffGetError::Failed(format!("merge-base failed for base {}", base.as_ref())));
    }

    let merge_base = String::from_utf8_lossy(&merge_base_output.stdout).trim().to_owned();

    let mut paths = BTreeSet::new();

    // 2. Committed diff from merge-base to HEAD
    let diff_output =
        run_git(&["diff", "--name-only", "-z", "--diff-filter=ACDMRT", &merge_base, "HEAD"])?;
    collect_paths(diff_output, "diff merge-base..HEAD", &mut paths)?;

    // 3. Staged but uncommitted
    let staged_output = run_git(&["diff", "--name-only", "-z", "--cached"])?;
    collect_paths(staged_output, "diff --cached", &mut paths)?;

    // 4. Unstaged worktree modifications
    let worktree_output = run_git(&["diff", "--name-only", "-z"])?;
    collect_paths(worktree_output, "diff (worktree)", &mut paths)?;

    // 5. Untracked (non-ignored) files
    let untracked_output = run_git(&["ls-files", "--others", "--exclude-standard", "-z"])?;
    collect_paths(untracked_output, "ls-files --others", &mut paths)?;

    Ok(paths.into_iter().collect())
}

/// Collects NUL-delimited, repository-relative paths from Git.
///
/// Git's `-z` form preserves literal newlines and other control bytes. The
/// review domain stores paths as UTF-8 strings, so a non-UTF-8 name is refused
/// rather than lossy-decoded into a different path and hashed as a tombstone.
fn collect_paths(
    output: Output,
    label: &str,
    paths: &mut BTreeSet<FilePath>,
) -> Result<(), DiffGetError> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(DiffGetError::Failed(format!(
            "{label} failed (exit {}): {stderr}",
            output.status.code().unwrap_or(-1)
        )));
    }
    for raw_path in output.stdout.split(|byte| *byte == b'\0') {
        if raw_path.is_empty() {
            continue;
        }
        let path = std::str::from_utf8(raw_path).map_err(|_| {
            DiffGetError::Failed(format!("{label}: git returned a non-UTF-8 repository path"))
        })?;
        let normalized = path.strip_prefix("./").unwrap_or(path);
        let file_path = FilePath::new(normalized).map_err(|error| {
            DiffGetError::Failed(format!("{label}: invalid repository path: {error}"))
        })?;
        paths.insert(file_path);
    }
    Ok(())
}

#[cfg(test)]
#[cfg(unix)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeSet;
    use std::os::unix::process::ExitStatusExt;
    use std::path::Path;
    use std::process::Command;
    use std::process::{ExitStatus, Output};

    use domain::CommitHash;

    use super::{collect_paths, list_diff_files_from_root};

    fn successful_output(stdout: Vec<u8>) -> Output {
        Output { status: ExitStatus::from_raw(0), stdout, stderr: Vec::new() }
    }

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git").args(args).current_dir(root).status().unwrap();
        assert!(status.success(), "git {:?} must succeed", args);
    }

    fn git_stdout(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git").args(args).current_dir(root).output().unwrap();
        assert!(output.status.success(), "git {:?} must succeed", args);
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn test_collect_paths_preserves_newline_in_nul_delimited_path() {
        let mut paths = BTreeSet::new();
        collect_paths(
            successful_output(b"src/line\nbreak.rs\0src/normal.rs\0".to_vec()),
            "diff",
            &mut paths,
        )
        .unwrap();

        assert!(paths.iter().any(|path| path.as_str() == "src/line\nbreak.rs"));
        assert!(paths.iter().any(|path| path.as_str() == "src/normal.rs"));
    }

    #[test]
    fn test_collect_paths_rejects_non_utf8_path() {
        let mut paths = BTreeSet::new();
        let error = collect_paths(
            successful_output(vec![b's', b'r', b'c', b'/', 0xff, 0]),
            "diff",
            &mut paths,
        )
        .unwrap_err();

        assert!(error.to_string().contains("non-UTF-8 repository path"));
    }

    #[test]
    fn test_list_diff_files_from_root_uses_the_bounded_isolated_git_lane() {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init"]);
        git(repo.path(), &["config", "user.email", "fixture@example.com"]);
        git(repo.path(), &["config", "user.name", "Fixture"]);
        git(repo.path(), &["commit", "--allow-empty", "-m", "base"]);
        let base = CommitHash::try_new(git_stdout(repo.path(), &["rev-parse", "HEAD"])).unwrap();
        std::fs::write(repo.path().join("changed.rs"), "changed").unwrap();

        let files = list_diff_files_from_root(repo.path(), &base).unwrap();

        assert!(files.iter().any(|path| path.as_str() == "changed.rs"));
    }
}
