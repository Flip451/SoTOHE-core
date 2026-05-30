//! Shared helpers used by multiple `semantic_dup` submodules.

use std::path::Path;

/// The subdirectory name LanceDB creates for the `fragments` table.
///
/// LanceDB stores each table as a `{table_name}.lance/` directory inside the
/// database root.  Checking for this marker lets us distinguish a genuine
/// LanceDB index from an arbitrary directory that the user accidentally
/// pointed `--db-path` at.
pub(super) const LANCEDB_TABLE_MARKER: &str = "fragments.lance";

/// Return `true` when `db_path` looks like a LanceDB index previously
/// created by this tool.
///
/// The check is intentionally conservative: a directory qualifies as a
/// recognizable index only when it contains the `fragments.lance/`
/// subdirectory that LanceDB creates for the `fragments` table.  The marker
/// must be a real directory (not a file or symlink) to avoid treating an
/// unrelated directory that happens to contain a same-named file or symlink
/// as a valid index.
///
/// `std::fs::symlink_metadata` is used deliberately (it does NOT follow
/// symlinks), so a `fragments.lance` symlink — even one pointing at a
/// directory — does NOT satisfy this check.  This prevents a data-loss bypass
/// where an attacker or accidental user creates a `fragments.lance` symlink
/// inside an unrelated directory to trick the guard into accepting it as a
/// recognizable index and subsequently deleting that directory.
pub(super) fn is_recognizable_lancedb_index(db_path: &Path) -> bool {
    match std::fs::symlink_metadata(db_path.join(LANCEDB_TABLE_MARKER)) {
        Ok(meta) => meta.file_type().is_dir(),
        Err(_) => false,
    }
}

/// Truncate `s` to at most `max_chars` characters, appending `…` if truncated.
pub(super) fn truncate_snippet(s: &str, max_chars: usize) -> String {
    let first_line = s.lines().next().unwrap_or("");
    if first_line.chars().count() <= max_chars {
        first_line.to_owned()
    } else {
        let truncated: String = first_line.chars().take(max_chars).collect();
        format!("{truncated}…")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_truncate_snippet_short_string_is_unchanged() {
        let s = "fn foo() {}";
        assert_eq!(truncate_snippet(s, 80), s);
    }

    #[test]
    fn test_truncate_snippet_long_string_is_truncated_with_ellipsis() {
        let s = "a".repeat(100);
        let result = truncate_snippet(&s, 10);
        assert!(result.ends_with('…'), "truncated snippet must end with '…'");
        // 10 chars + 1 `…` multibyte character = chars count 11.
        assert_eq!(result.chars().count(), 11);
    }

    #[test]
    fn test_truncate_snippet_uses_only_first_line() {
        let s = "first line\nsecond line\nthird line";
        let result = truncate_snippet(s, 80);
        assert_eq!(result, "first line");
    }
}
