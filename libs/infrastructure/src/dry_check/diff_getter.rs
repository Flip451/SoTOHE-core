//! `GitDryCheckDiffGetter` — dry-check's own git diff adapter.
//!
//! Implements `usecase::dry_check::DryCheckDiffSource` (CN-01: this is NOT
//! `GitDiffGetter` from `review_v2`). Returns `Vec<DiffFileHunks>` (each
//! carrying a `FilePath` and a non-empty `Vec<DiffHunkRange>`) instead of bare
//! `Vec<FilePath>`, enabling hunk-level overlap detection (D4 hunk-scope).
//!
//! Behavior mirrors `GitDiffGetter`: 4-source union via `SystemGitRepo`:
//! 1. `git diff --unified=0 <merge-base> HEAD` (committed diff hunk ranges)
//! 2. `git diff --unified=0 --cached` (staged hunk ranges)
//! 3. `git diff --unified=0` (unstaged worktree hunk ranges)
//! 4. `git ls-files --others --exclude-standard` (untracked files — whole-file hunk)

use std::collections::BTreeMap;

use domain::CommitHash;
use domain::dry_check::{DiffFileHunks, DiffHunkRange, DiffHunkRangeError};
use domain::review_v2::FilePath;
use usecase::dry_check::{DryCheckDiffError, DryCheckDiffSource};

use crate::git_cli::{GitRepository as _, SystemGitRepo};

// ── GitDryCheckDiffGetter ─────────────────────────────────────────────────────

/// Git-based adapter implementing dry-check's own diff-source port.
///
/// Uses `git diff --unified=0` to extract hunk-level line ranges for each
/// changed file. Returns a deduplicated union from 4 sources (mirroring
/// `GitDiffGetter`), but with hunk ranges instead of bare file paths.
#[derive(Debug)]
pub struct GitDryCheckDiffGetter;

impl DryCheckDiffSource for GitDryCheckDiffGetter {
    fn list_changed_hunks(
        &self,
        base: &CommitHash,
    ) -> Result<Vec<DiffFileHunks>, DryCheckDiffError> {
        let git = SystemGitRepo::discover()
            .map_err(|e| DryCheckDiffError::Failed(format!("git discover: {e}")))?;

        // Find merge-base between HEAD and base commit.
        let merge_base_output = git
            .output(&["merge-base", "HEAD", base.as_ref()])
            .map_err(|e| DryCheckDiffError::Failed(format!("merge-base: {e}")))?;

        if !merge_base_output.status.success() {
            return Err(DryCheckDiffError::Failed(format!(
                "merge-base failed for base {}",
                base.as_ref()
            )));
        }

        let merge_base = String::from_utf8_lossy(&merge_base_output.stdout).trim().to_owned();

        // file_path → list of hunk ranges (accumulate across all 4 sources).
        let mut hunk_map: BTreeMap<String, Vec<DiffHunkRange>> = BTreeMap::new();

        // Source 1: committed diff from merge-base to HEAD.
        collect_hunk_ranges(
            &git,
            &["diff", "--unified=0", "--diff-filter=ACDMRT", &merge_base, "HEAD"],
            "diff merge-base..HEAD",
            &mut hunk_map,
        )?;

        // Source 2: staged but uncommitted.
        collect_hunk_ranges(
            &git,
            &["diff", "--unified=0", "--cached"],
            "diff --cached",
            &mut hunk_map,
        )?;

        // Source 3: unstaged worktree modifications.
        collect_hunk_ranges(&git, &["diff", "--unified=0"], "diff (worktree)", &mut hunk_map)?;

        // Source 4: untracked files — treat as whole-file (line 1 to u32::MAX).
        collect_untracked_files(&git, &mut hunk_map)?;

        // Convert map to Vec<DiffFileHunks>, skipping empty-hunk entries.
        let mut result = Vec::new();
        for (path_str, hunks) in hunk_map {
            if hunks.is_empty() {
                continue;
            }
            let file_path = FilePath::new(&path_str).map_err(|e| {
                DryCheckDiffError::Failed(format!("invalid path '{path_str}': {e}"))
            })?;
            if let Ok(dfh) = DiffFileHunks::new(file_path, hunks) {
                result.push(dfh);
            }
            // Err(EmptyHunks) → skip (structurally enforced)
        }

        Ok(result)
    }
}

/// Parse `git diff --unified=0` output and accumulate hunk line ranges into `map`.
///
/// For each `+<start>[,<count>]` hunk header in the `@@` lines, computes an
/// inclusive `[start, start + count - 1]` range for the **new** file side.
/// Lines added to a file are the ones we care about for overlap detection.
fn collect_hunk_ranges(
    git: &SystemGitRepo,
    args: &[&str],
    label: &str,
    map: &mut BTreeMap<String, Vec<DiffHunkRange>>,
) -> Result<(), DryCheckDiffError> {
    let output =
        git.output(args).map_err(|e| DryCheckDiffError::Failed(format!("{label}: {e}")))?;

    if !output.status.success() {
        return Err(command_failed(label, &output));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current_file: Option<String> = None;
    let mut pending_old_file_header = false;
    let mut in_hunk = false;

    for line in stdout.lines() {
        if line.starts_with("diff --git ") {
            pending_old_file_header = false;
            in_hunk = false;
            continue;
        }

        if !in_hunk && (line.starts_with("--- a/") || line == "--- /dev/null") {
            pending_old_file_header = true;
            continue;
        }

        if !in_hunk && pending_old_file_header {
            pending_old_file_header = false;
            if let Some(path) = line.strip_prefix("+++ b/") {
                let path = path.trim_end_matches(['\n', '\r']);
                current_file = Some(path.to_owned());
                map.entry(path.to_owned()).or_default();
                continue;
            }
            if line == "+++ /dev/null" {
                current_file = None;
                continue;
            }
        }

        if line.starts_with("@@ ") {
            in_hunk = true;
            // Hunk header: `@@ -<old> +<new_start>[,<count>] @@`
            if let Some(ref path) = current_file {
                if let Some(range) = parse_hunk_new_range(line) {
                    map.entry(path.clone()).or_default().push(range);
                }
            }
        }
    }

    Ok(())
}

/// Collect untracked files and insert them as whole-file hunks (line 1..=u32::MAX).
fn collect_untracked_files(
    git: &SystemGitRepo,
    map: &mut BTreeMap<String, Vec<DiffHunkRange>>,
) -> Result<(), DryCheckDiffError> {
    let output = git
        .output(&["ls-files", "--others", "--exclude-standard"])
        .map_err(|e| DryCheckDiffError::Failed(format!("ls-files: {e}")))?;

    if !output.status.success() {
        return Err(command_failed("ls-files --others", &output));
    }

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let path = line.trim_end_matches(['\n', '\r']);
        let normalized = path.strip_prefix("./").unwrap_or(path);
        if normalized.is_empty() {
            continue;
        }
        // Whole-file range: 1..=u32::MAX (sentinel for unknown line count).
        let range = DiffHunkRange::new(1, u32::MAX)
            .map_err(|e| DryCheckDiffError::Failed(format!("hunk range for {normalized}: {e}")))?;
        map.entry(normalized.to_owned()).or_default().push(range);
    }

    Ok(())
}

fn command_failed(label: &str, output: &std::process::Output) -> DryCheckDiffError {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let code =
        output.status.code().map_or_else(|| "unknown".to_owned(), |status| status.to_string());
    DryCheckDiffError::Failed(format!("{label} failed (exit {code}): {stderr}"))
}

/// Parse the new-file side of a hunk header `@@ -<old> +<start>[,<count>] @@ ...`.
///
/// Returns `None` for deletions-only hunks (`+0,0`) and invalid lines.
/// Returns a [`DiffHunkRange`] `[start, start + count - 1]` (inclusive) for
/// insertions and modifications.
fn parse_hunk_new_range(hunk_line: &str) -> Option<DiffHunkRange> {
    // Parse `@@ -L,S +L,S @@ ...` format.
    let new_part = parse_new_side(hunk_line)?;

    let (start_str, count_str) = if let Some((start, count)) = new_part.split_once(',') {
        (start, count)
    } else {
        (new_part, "1")
    };

    let start: u32 = start_str.parse().ok()?;
    let count: u32 = count_str.parse().ok()?;

    if start == 0 || count == 0 {
        // Deletion-only hunk or invalid — no new lines.
        return None;
    }

    let end = start.saturating_add(count).saturating_sub(1);
    match DiffHunkRange::new(start, end) {
        Ok(r) => Some(r),
        Err(DiffHunkRangeError::ZeroLine) | Err(DiffHunkRangeError::StartExceedsEnd { .. }) => None,
    }
}

/// Extract the `<start>[,<count>]` string from the new-file (`+`) side of a hunk header.
fn parse_new_side(hunk_line: &str) -> Option<&str> {
    // `@@ -<old_start>[,<old_count>] +<new_start>[,<new_count>] @@ ...`
    // Find `+` after the `-` part.
    let after_at = hunk_line.strip_prefix("@@ -")?;
    // Skip past old part (up to space).
    let space_pos = after_at.find(' ')?;
    let rest = after_at.get(space_pos + 1..)?;
    let new_part = rest.strip_prefix('+')?;
    // new_part now starts with `<start>[,<count>]` followed by ` @@`.
    let end = new_part.find(' ').unwrap_or(new_part.len());
    new_part.get(..end)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use super::parse_hunk_new_range;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    struct CurrentDirGuard {
        original: PathBuf,
    }

    impl CurrentDirGuard {
        fn enter(path: &Path) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { original }
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let output =
            std::process::Command::new("git").args(args).current_dir(dir).output().unwrap();
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    #[test]
    fn test_parse_hunk_new_range_single_line() {
        // `@@ -1 +1 @@` — one line changed at line 1
        let range = parse_hunk_new_range("@@ -1 +1 @@").unwrap();
        assert_eq!(range.start_line(), 1);
        assert_eq!(range.end_line(), 1);
    }

    #[test]
    fn test_parse_hunk_new_range_multiple_lines() {
        // `@@ -1,4 +1,6 @@` — 6 lines starting at line 1
        let range = parse_hunk_new_range("@@ -1,4 +1,6 @@").unwrap();
        assert_eq!(range.start_line(), 1);
        assert_eq!(range.end_line(), 6);
    }

    #[test]
    fn test_parse_hunk_new_range_deletion_only_returns_none() {
        // `@@ -1,3 +1,0 @@` — deletion only, count=0
        let result = parse_hunk_new_range("@@ -1,3 +1,0 @@");
        assert!(result.is_none(), "deletion-only hunk should return None");
    }

    #[test]
    fn test_parse_hunk_new_range_start_zero_returns_none() {
        // `@@ -1,0 +0,0 @@` — no new lines
        let result = parse_hunk_new_range("@@ -1,0 +0,0 @@");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_hunk_new_range_with_context_comment() {
        // Real git diff hunk header with context
        let range = parse_hunk_new_range("@@ -10,3 +15,5 @@ fn example() {").unwrap();
        assert_eq!(range.start_line(), 15);
        assert_eq!(range.end_line(), 19); // 15 + 5 - 1 = 19
    }

    #[test]
    fn test_git_dry_check_diff_getter_unions_all_four_sources() {
        use domain::CommitHash;
        use usecase::dry_check::DryCheckDiffSource;

        let _lock = CWD_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init"]);
        git(dir.path(), &["config", "user.email", "test@example.com"]);
        git(dir.path(), &["config", "user.name", "Test User"]);

        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let file = src_dir.join("lib.rs");
        std::fs::write(&file, "fn existing() {}\n").unwrap();
        git(dir.path(), &["add", "src/lib.rs"]);
        git(dir.path(), &["commit", "-m", "base"]);
        let base = CommitHash::try_new(git(dir.path(), &["rev-parse", "HEAD"])).unwrap();

        std::fs::write(src_dir.join("committed.rs"), "fn committed() {}\n").unwrap();
        git(dir.path(), &["add", "src/committed.rs"]);
        git(dir.path(), &["commit", "-m", "committed change"]);

        std::fs::write(src_dir.join("staged.rs"), "fn staged() {}\n").unwrap();
        git(dir.path(), &["add", "src/staged.rs"]);

        std::fs::write(&file, "fn existing() {}\n++ foo\n").unwrap();
        std::fs::write(src_dir.join("untracked.rs"), "fn untracked() {}\n").unwrap();

        let _cwd = CurrentDirGuard::enter(dir.path());
        let getter = super::GitDryCheckDiffGetter;
        let result = getter.list_changed_hunks(&base).unwrap();

        for expected in ["src/committed.rs", "src/staged.rs", "src/lib.rs", "src/untracked.rs"] {
            assert!(
                result.iter().any(|entry| entry.path().as_str() == expected),
                "missing expected diff source path: {expected}; got {result:?}"
            );
        }

        let unstaged_hunks =
            result.iter().find(|entry| entry.path().as_str() == "src/lib.rs").unwrap();
        let hunk = unstaged_hunks.hunks().first().unwrap();
        assert_eq!(hunk.start_line(), 2);
        assert_eq!(hunk.end_line(), 2);

        let untracked_hunks =
            result.iter().find(|entry| entry.path().as_str() == "src/untracked.rs").unwrap();
        let whole_file = untracked_hunks.hunks().first().unwrap();
        assert_eq!(whole_file.start_line(), 1);
        assert_eq!(whole_file.end_line(), u32::MAX);
    }

    #[test]
    fn test_git_dry_check_diff_getter_returns_error_when_merge_base_fails() {
        use domain::CommitHash;
        use usecase::dry_check::DryCheckDiffSource;

        let _lock = CWD_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init"]);

        let _cwd = CurrentDirGuard::enter(dir.path());
        let getter = super::GitDryCheckDiffGetter;
        let base = CommitHash::try_new("abcdef1").unwrap();
        let result = getter.list_changed_hunks(&base);

        assert!(result.is_err(), "invalid merge-base should return an error");
    }
}
