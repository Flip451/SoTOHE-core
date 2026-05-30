//! Shared helpers used by multiple `semantic_dup` submodules.

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
