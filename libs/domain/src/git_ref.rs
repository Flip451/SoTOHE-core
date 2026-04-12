//! Validation rules for git branch references used in fail-closed gates.
//!
//! `validate_branch_ref` rejects branch names that contain characters git would
//! interpret as ref-range (`..`), reflog expression (`@{`), ancestor (`~` / `^`),
//! path separator (`:`), whitespace, or other control characters. It is called
//! by the merge gate and task-completion gate (via the `TrackBlobReader` port)
//! before constructing `origin/{branch}:path` strings, to prevent fail-open
//! behavior from mis-parsed refs.
//!
//! Reference: ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md`
//! §D2.0, §D4.2.

use thiserror::Error;

/// Errors returned by `validate_branch_ref`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RefValidationError {
    /// The branch name is empty.
    #[error("branch name is empty")]
    Empty,
    /// The branch name contains a character or substring that git would
    /// mis-interpret as a ref-range, reflog expression, ancestor, or separator.
    #[error("branch name contains disallowed character or sequence: {0}")]
    DisallowedCharacter(String),
}

/// Validates a branch name for use in `origin/{branch}:{path}` git-ref strings.
///
/// Rejects:
/// - empty string
/// - `..` (ref range)
/// - `@{` (reflog expression)
/// - `~` (ancestor)
/// - `^` (ancestor)
/// - `:` (already used as the blob separator in the outer string)
/// - whitespace characters (space, tab, newline, etc.)
/// - ASCII control characters
///
/// Other characters (letters, digits, `-`, `_`, `/`, `.`) are accepted.
///
/// This is a pure function — it does not touch the filesystem or spawn any
/// process. It is called by the `usecase::merge_gate` and `usecase::task_completion`
/// modules before calling into the `TrackBlobReader` port adapter.
///
/// # Errors
///
/// Returns [`RefValidationError::Empty`] when the input is an empty string, or
/// [`RefValidationError::DisallowedCharacter`] when any of the disallowed
/// characters or sequences are present.
///
/// # Examples
///
/// ```
/// use domain::git_ref::validate_branch_ref;
///
/// assert!(validate_branch_ref("track/strict-signal-gate-v2-2026-04-12").is_ok());
/// assert!(validate_branch_ref("plan/my-feature").is_ok());
/// assert!(validate_branch_ref("").is_err());
/// assert!(validate_branch_ref("feature/foo..bar").is_err());
/// assert!(validate_branch_ref("feature/foo@{0}").is_err());
/// ```
pub fn validate_branch_ref(branch: &str) -> Result<(), RefValidationError> {
    if branch.is_empty() {
        return Err(RefValidationError::Empty);
    }

    // Substring-based checks for multi-char sequences.
    if branch.contains("..") {
        return Err(RefValidationError::DisallowedCharacter("..".to_owned()));
    }
    if branch.contains("@{") {
        return Err(RefValidationError::DisallowedCharacter("@{".to_owned()));
    }

    // Per-character checks.
    for ch in branch.chars() {
        if ch == '~' {
            return Err(RefValidationError::DisallowedCharacter("~".to_owned()));
        }
        if ch == '^' {
            return Err(RefValidationError::DisallowedCharacter("^".to_owned()));
        }
        if ch == ':' {
            return Err(RefValidationError::DisallowedCharacter(":".to_owned()));
        }
        if ch.is_whitespace() {
            return Err(RefValidationError::DisallowedCharacter(format!(
                "whitespace (U+{:04X})",
                ch as u32
            )));
        }
        if ch.is_control() {
            return Err(RefValidationError::DisallowedCharacter(format!(
                "control character (U+{:04X})",
                ch as u32
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // --- D14: valid branch name accepted ---

    #[test]
    fn test_validate_branch_ref_accepts_normal_track_branch() {
        assert!(validate_branch_ref("track/strict-signal-gate-v2-2026-04-12").is_ok());
    }

    #[test]
    fn test_validate_branch_ref_accepts_plan_branch() {
        assert!(validate_branch_ref("plan/my-feature").is_ok());
    }

    #[test]
    fn test_validate_branch_ref_accepts_main() {
        assert!(validate_branch_ref("main").is_ok());
    }

    #[test]
    fn test_validate_branch_ref_accepts_feature_branch_with_digits() {
        assert!(validate_branch_ref("feature/user-123/sub-branch").is_ok());
    }

    // --- D15: `..` rejected ---

    #[test]
    fn test_validate_branch_ref_rejects_ref_range() {
        let err = validate_branch_ref("feature/foo..bar").unwrap_err();
        assert_eq!(err, RefValidationError::DisallowedCharacter("..".to_owned()));
    }

    // --- D16: `@{` rejected ---

    #[test]
    fn test_validate_branch_ref_rejects_reflog_expression() {
        let err = validate_branch_ref("feature/foo@{0}").unwrap_err();
        assert_eq!(err, RefValidationError::DisallowedCharacter("@{".to_owned()));
    }

    // --- D17: whitespace rejected ---

    #[test]
    fn test_validate_branch_ref_rejects_space() {
        let err = validate_branch_ref("feature with space").unwrap_err();
        assert!(
            matches!(err, RefValidationError::DisallowedCharacter(ref s) if s.contains("whitespace"))
        );
    }

    #[test]
    fn test_validate_branch_ref_rejects_tab() {
        let err = validate_branch_ref("feature\tfoo").unwrap_err();
        assert!(
            matches!(err, RefValidationError::DisallowedCharacter(ref s) if s.contains("whitespace"))
        );
    }

    // --- D18: `~` rejected ---

    #[test]
    fn test_validate_branch_ref_rejects_tilde() {
        let err = validate_branch_ref("feature/foo~1").unwrap_err();
        assert_eq!(err, RefValidationError::DisallowedCharacter("~".to_owned()));
    }

    // --- D19: `^` rejected ---

    #[test]
    fn test_validate_branch_ref_rejects_caret() {
        let err = validate_branch_ref("feature/foo^").unwrap_err();
        assert_eq!(err, RefValidationError::DisallowedCharacter("^".to_owned()));
    }

    // --- D20: empty string rejected ---

    #[test]
    fn test_validate_branch_ref_rejects_empty() {
        let err = validate_branch_ref("").unwrap_err();
        assert_eq!(err, RefValidationError::Empty);
    }

    // --- D21: `:` rejected ---

    #[test]
    fn test_validate_branch_ref_rejects_colon() {
        let err = validate_branch_ref("feature/foo:bar").unwrap_err();
        assert_eq!(err, RefValidationError::DisallowedCharacter(":".to_owned()));
    }

    // --- D22: control character rejected ---

    #[test]
    fn test_validate_branch_ref_rejects_control_character() {
        let err = validate_branch_ref("feature/foo\u{0001}bar").unwrap_err();
        assert!(
            matches!(err, RefValidationError::DisallowedCharacter(ref s) if s.contains("control"))
        );
    }

    #[test]
    fn test_validate_branch_ref_rejects_newline() {
        // Newline is both whitespace and control — whitespace check fires first
        let err = validate_branch_ref("feature/foo\nbar").unwrap_err();
        assert!(
            matches!(err, RefValidationError::DisallowedCharacter(ref s) if s.contains("whitespace"))
        );
    }
}
