//! Worktree cleanliness validation logic extracted from CLI layer.
//!
//! These functions parse git porcelain output and validate worktree state
//! against an allowlist of expected dirty paths. The actual git command
//! execution remains in the CLI/infrastructure layer.

use std::collections::BTreeSet;

use domain::WorktreeReader;

/// Parses `git status --porcelain` output into a list of dirty file paths.
///
/// Each line of porcelain output has the format `XY path` (3-char prefix + path).
/// Rename entries (`old -> new`) are normalized to the destination path.
#[must_use]
pub fn parse_dirty_worktree_paths(porcelain_output: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in porcelain_output.lines() {
        if line.len() < 4 {
            continue;
        }
        let path = &line[3..];
        let normalized = path.split_once(" -> ").map(|(_, after)| after).unwrap_or(path).trim();
        if !normalized.is_empty() {
            paths.push(normalized.to_owned());
        }
    }
    paths
}

/// Validates that the worktree is clean enough for activation.
///
/// Returns `Ok(())` if all dirty paths are in the allowed set, or if there
/// are no dirty paths at all.
///
/// # Errors
/// Returns an error message if any dirty path is not in the allowed set.
pub fn validate_clean_worktree(
    dirty_paths: &[String],
    allowed_dirty_paths: &BTreeSet<String>,
) -> Result<(), String> {
    if dirty_paths.is_empty() {
        return Ok(());
    }
    if dirty_paths.iter().all(|path| allowed_dirty_paths.contains(path)) {
        return Ok(());
    }
    Err("activation requires a clean worktree before metadata materialization".to_owned())
}

/// Checks worktree cleanliness via a `WorktreeReader` port.
///
/// Reads porcelain status from the reader, parses dirty paths, and validates
/// against the allowed set. This integrates the I/O and validation steps that
/// were previously split across CLI and usecase layers.
///
/// # Errors
/// Returns an error message if the worktree has disallowed dirty paths
/// or if the porcelain status cannot be read.
pub fn ensure_clean_worktree(
    reader: &impl WorktreeReader,
    allowed_dirty_paths: &BTreeSet<String>,
) -> Result<(), String> {
    let porcelain = reader.porcelain_status()?;
    let dirty_paths = parse_dirty_worktree_paths(&porcelain);
    validate_clean_worktree(&dirty_paths, allowed_dirty_paths)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // --- parse_dirty_worktree_paths ---

    #[test]
    fn test_parse_dirty_worktree_paths_with_empty_output_returns_empty() {
        let result = parse_dirty_worktree_paths("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_dirty_worktree_paths_with_modified_file_returns_path() {
        let result = parse_dirty_worktree_paths(" M src/main.rs\n");
        assert_eq!(result, vec!["src/main.rs"]);
    }

    #[test]
    fn test_parse_dirty_worktree_paths_with_rename_returns_destination() {
        let result = parse_dirty_worktree_paths("R  old.rs -> new.rs\n");
        assert_eq!(result, vec!["new.rs"]);
    }

    #[test]
    fn test_parse_dirty_worktree_paths_with_multiple_files_returns_all() {
        let output = " M src/a.rs\n?? src/b.rs\nA  src/c.rs\n";
        let result = parse_dirty_worktree_paths(output);
        assert_eq!(result, vec!["src/a.rs", "src/b.rs", "src/c.rs"]);
    }

    #[test]
    fn test_parse_dirty_worktree_paths_skips_short_lines() {
        let result = parse_dirty_worktree_paths("ab\n M ok.rs\n");
        assert_eq!(result, vec!["ok.rs"]);
    }

    // --- validate_clean_worktree ---

    #[test]
    fn test_validate_clean_worktree_with_no_dirty_paths_succeeds() {
        let result = validate_clean_worktree(&[], &BTreeSet::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_clean_worktree_with_all_allowed_succeeds() {
        let dirty = vec!["track/items/x/metadata.json".to_owned()];
        let allowed = BTreeSet::from(["track/items/x/metadata.json".to_owned()]);
        let result = validate_clean_worktree(&dirty, &allowed);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_clean_worktree_with_disallowed_path_returns_error() {
        let dirty = vec!["src/main.rs".to_owned()];
        let allowed = BTreeSet::from(["track/items/x/metadata.json".to_owned()]);
        let result = validate_clean_worktree(&dirty, &allowed);
        assert!(result.unwrap_err().contains("clean worktree"));
    }

    #[test]
    fn test_validate_clean_worktree_with_mixed_paths_returns_error() {
        let dirty = vec!["track/items/x/metadata.json".to_owned(), "src/main.rs".to_owned()];
        let allowed = BTreeSet::from(["track/items/x/metadata.json".to_owned()]);
        let result = validate_clean_worktree(&dirty, &allowed);
        assert!(result.is_err());
    }

    // --- ensure_clean_worktree (with WorktreeReader) ---

    struct StubWorktreeReader {
        output: String,
    }

    impl domain::WorktreeReader for StubWorktreeReader {
        fn porcelain_status(&self) -> Result<String, String> {
            Ok(self.output.clone())
        }
    }

    struct FailingWorktreeReader;

    impl domain::WorktreeReader for FailingWorktreeReader {
        fn porcelain_status(&self) -> Result<String, String> {
            Err("git failed".to_owned())
        }
    }

    #[test]
    fn test_ensure_clean_worktree_with_clean_worktree_succeeds() {
        let reader = StubWorktreeReader { output: String::new() };
        let result = ensure_clean_worktree(&reader, &BTreeSet::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_ensure_clean_worktree_with_allowed_dirty_paths_succeeds() {
        let reader = StubWorktreeReader { output: " M track/items/x/metadata.json\n".to_owned() };
        let allowed = BTreeSet::from(["track/items/x/metadata.json".to_owned()]);
        let result = ensure_clean_worktree(&reader, &allowed);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ensure_clean_worktree_with_disallowed_dirty_paths_returns_error() {
        let reader = StubWorktreeReader { output: " M src/main.rs\n".to_owned() };
        let result = ensure_clean_worktree(&reader, &BTreeSet::new());
        assert!(result.unwrap_err().contains("clean worktree"));
    }

    #[test]
    fn test_ensure_clean_worktree_propagates_reader_error() {
        let reader = FailingWorktreeReader;
        let result = ensure_clean_worktree(&reader, &BTreeSet::new());
        assert!(result.unwrap_err().contains("git failed"));
    }
}
