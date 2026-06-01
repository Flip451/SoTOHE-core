pub(super) fn is_forbidden_sandbox_value(val: &str) -> bool {
    matches!(val, "danger-full-access" | "dangerously-bypass-approvals-and-sandbox")
}

pub(super) fn parse_semver_from_text(text: &str) -> Option<String> {
    for word in text.split_whitespace() {
        let candidate = word.trim_matches(|c: char| !c.is_ascii_digit());
        let parts: Vec<&str> = candidate.split('.').collect();
        let valid = parts.first().and_then(|p| p.parse::<u64>().ok()).is_some()
            && parts.get(1).and_then(|p| p.parse::<u64>().ok()).is_some()
            && parts.get(2).is_some_and(|p| p.chars().take_while(char::is_ascii_digit).count() > 0);
        if parts.len() >= 3 && valid {
            return Some(candidate.to_owned());
        }
    }
    None
}

pub(super) fn parse_major_minor(version: &str) -> Option<(u64, u64)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    Some((major, minor))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // ── is_forbidden_sandbox_value ────────────────────────────────────────────
    //
    // The `#![forbid(unsafe_code)]` crate attribute prevents calling
    // `std::env::set_var` / `remove_var` (unsafe in Rust 2024) from tests.
    // We test the AC-07 requirement by exercising the pure helper
    // `is_forbidden_sandbox_value` directly — the same function the method
    // delegates to — rather than mutating the environment.

    #[test]
    fn test_is_forbidden_sandbox_value_danger_full_access_returns_true() {
        assert!(
            is_forbidden_sandbox_value("danger-full-access"),
            "danger-full-access must be identified as forbidden"
        );
    }

    #[test]
    fn test_is_forbidden_sandbox_value_dangerously_bypass_returns_true() {
        assert!(
            is_forbidden_sandbox_value("dangerously-bypass-approvals-and-sandbox"),
            "dangerously-bypass-approvals-and-sandbox must be identified as forbidden"
        );
    }

    #[test]
    fn test_is_forbidden_sandbox_value_workspace_write_returns_false() {
        assert!(
            !is_forbidden_sandbox_value("workspace-write"),
            "workspace-write must NOT be forbidden"
        );
    }

    #[test]
    fn test_is_forbidden_sandbox_value_read_only_returns_false() {
        assert!(!is_forbidden_sandbox_value("read-only"), "read-only must NOT be forbidden");
    }

    #[test]
    fn test_is_forbidden_sandbox_value_empty_returns_false() {
        assert!(!is_forbidden_sandbox_value(""), "empty string must NOT be forbidden");
    }

    // ── parse_semver_from_text ────────────────────────────────────────────────

    #[test]
    fn test_parse_semver_from_text_finds_version_in_typical_output() {
        let output = "codex 0.125.0 (abc123)";
        assert_eq!(parse_semver_from_text(output).as_deref(), Some("0.125.0"));
    }

    #[test]
    fn test_parse_semver_from_text_returns_none_for_empty() {
        assert!(parse_semver_from_text("").is_none());
    }

    #[test]
    fn test_parse_semver_from_text_returns_none_for_non_version_text() {
        assert!(parse_semver_from_text("no version here at all").is_none());
    }
}
