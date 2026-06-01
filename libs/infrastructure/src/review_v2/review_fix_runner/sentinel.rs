pub(super) fn parse_sentinel(output: &str) -> Option<&'static str> {
    let last_line = output.lines().rev().find(|line| !line.trim().is_empty())?;
    match last_line {
        "REVIEW_FIX_STATUS: completed" => Some("completed"),
        "REVIEW_FIX_STATUS: blocked_cross_scope" => Some("blocked_cross_scope"),
        "REVIEW_FIX_STATUS: failed" => Some("failed"),
        _ => None,
    }
}

pub(super) fn sentinel_to_exit_code(status: &str) -> i32 {
    match status {
        "completed" => 0,
        "blocked_cross_scope" => 2,
        _ => 1,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sentinel_completed_returns_completed() {
        let output = "some output\nREVIEW_FIX_STATUS: completed";
        assert_eq!(parse_sentinel(output), Some("completed"));
    }

    #[test]
    fn test_parse_sentinel_blocked_cross_scope_returns_blocked_cross_scope() {
        let output = "some output\nREVIEW_FIX_STATUS: blocked_cross_scope";
        assert_eq!(parse_sentinel(output), Some("blocked_cross_scope"));
    }

    #[test]
    fn test_parse_sentinel_failed_returns_failed() {
        let output = "some output\nREVIEW_FIX_STATUS: failed";
        assert_eq!(parse_sentinel(output), Some("failed"));
    }

    #[test]
    fn test_parse_sentinel_empty_output_returns_none() {
        assert_eq!(parse_sentinel(""), None);
    }

    #[test]
    fn test_parse_sentinel_whitespace_only_returns_none() {
        assert_eq!(parse_sentinel("   \n\n  "), None);
    }

    #[test]
    fn test_parse_sentinel_embedded_in_prose_not_last_line_returns_none() {
        // Sentinel embedded in prose on a non-last line must NOT match.
        let output = "REVIEW_FIX_STATUS: completed — but here is more text explaining things\n\
             followed by trailing lines that are not the sentinel";
        assert_eq!(parse_sentinel(output), None);
    }

    #[test]
    fn test_parse_sentinel_sentinel_with_trailing_text_does_not_match() {
        // Line has extra text after the sentinel value — must NOT match.
        let output = "REVIEW_FIX_STATUS: completed and some extra text";
        assert_eq!(parse_sentinel(output), None);
    }

    #[test]
    fn test_parse_sentinel_sentinel_with_trailing_space_does_not_match() {
        let output = "REVIEW_FIX_STATUS: completed ";
        assert_eq!(parse_sentinel(output), None);
    }

    #[test]
    fn test_parse_sentinel_sentinel_with_leading_space_does_not_match() {
        let output = " REVIEW_FIX_STATUS: completed";
        assert_eq!(parse_sentinel(output), None);
    }

    #[test]
    fn test_parse_sentinel_trailing_text_line_after_sentinel_returns_none() {
        let output = "REVIEW_FIX_STATUS: completed\nextra text after sentinel";
        assert_eq!(parse_sentinel(output), None);
    }

    #[test]
    fn test_parse_sentinel_trailing_blank_lines_do_not_mask_sentinel() {
        // Trailing blank or whitespace-only lines must not cause the sentinel to be missed.
        let output = "REVIEW_FIX_STATUS: completed\n  \n\t\n";
        assert_eq!(parse_sentinel(output), Some("completed"));
    }

    #[test]
    fn test_parse_sentinel_codex_footer_after_sentinel_returns_none() {
        let output = "some preamble\n\
             REVIEW_FIX_STATUS: completed\n\
             \n\
             [tokens: prompt=12345 completion=678 total=13023]";
        assert_eq!(parse_sentinel(output), None);
    }

    #[test]
    fn test_parse_sentinel_last_sentinel_wins_when_multiple_present() {
        // When the sentinel appears multiple times, the last occurrence wins.
        let output = "REVIEW_FIX_STATUS: failed\nsome text\nREVIEW_FIX_STATUS: completed";
        assert_eq!(parse_sentinel(output), Some("completed"));
    }

    // ── sentinel_to_exit_code ─────────────────────────────────────────────────

    #[test]
    fn test_sentinel_to_exit_code_completed_is_zero() {
        assert_eq!(sentinel_to_exit_code("completed"), 0);
    }

    #[test]
    fn test_sentinel_to_exit_code_blocked_cross_scope_is_two() {
        assert_eq!(sentinel_to_exit_code("blocked_cross_scope"), 2);
    }

    #[test]
    fn test_sentinel_to_exit_code_failed_is_one() {
        assert_eq!(sentinel_to_exit_code("failed"), 1);
    }
}
