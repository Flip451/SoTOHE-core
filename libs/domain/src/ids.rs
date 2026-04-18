//! Validated domain identifier newtypes.
//!
//! All six types are hand-written plain structs rather than `nutype`-generated
//! wrappers. The current `schema_export` + rustdoc JSON pipeline does not follow
//! `pub use` aliases that point into `#[doc(hidden)]` modules (which is how
//! `nutype` publishes its generated structs), so `nutype`-wrapped types silently
//! disappear from the TDDD schema and are mis-classified as Yellow. Using plain
//! structs keeps every identifier visible to the current schema-export path and
//! obviates the need for a separate `harness-hardening-nutype-rustdoc-support`
//! follow-up track.
//!
//! Each type preserves the same public API previously offered by the
//! `nutype`-generated variants: `try_new(impl Into<String>) -> Result<Self,
//! ValidationError>`, `AsRef<str>`, `Display`, and the full derive group
//! (`Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord`).
//!
//! `NonEmptyString` and `ReviewGroupName` continue to honour the previous
//! `sanitize(trim)` behaviour by trimming leading/trailing whitespace before
//! running the non-empty validator.

use std::fmt;

use crate::ValidationError;

/// A validated track identifier (lowercase slug format).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(String);

impl TrackId {
    /// Validate and wrap `value` as a [`TrackId`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidTrackId`] when `value` is not a valid
    /// track id (must start with a lowercase ASCII letter or digit, then use
    /// lowercase letters / digits / single-hyphen separators, and must not end
    /// with a hyphen).
    pub fn try_new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if is_valid_track_id(&value) {
            Ok(Self(value))
        } else {
            Err(ValidationError::InvalidTrackId(value))
        }
    }
}

impl AsRef<str> for TrackId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TrackId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated task identifier (format: `T` followed by one or more digits).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(String);

impl TaskId {
    /// Validate and wrap `value` as a [`TaskId`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidTaskId`] when `value` does not match
    /// the `T<digits>` pattern or the digit portion cannot be parsed as `u64`.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if is_valid_task_id(&value) {
            Ok(Self(value))
        } else {
            Err(ValidationError::InvalidTaskId(value))
        }
    }
}

impl AsRef<str> for TaskId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated git commit hash (7–40 lowercase hex characters).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommitHash(String);

impl CommitHash {
    /// Validate and wrap `value` as a [`CommitHash`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidCommitHash`] when `value` is not 7–40
    /// lowercase ASCII hexadecimal characters.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if is_valid_commit_hash(&value) {
            Ok(Self(value))
        } else {
            Err(ValidationError::InvalidCommitHash(value))
        }
    }
}

impl AsRef<str> for CommitHash {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommitHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated track branch name (format: `track/<valid-track-id>`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackBranch(String);

impl TrackBranch {
    /// Validate and wrap `value` as a [`TrackBranch`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidTrackBranch`] when `value` is not in
    /// `track/<valid-track-id>` form.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if let Some(slug) = value.strip_prefix("track/") {
            if is_valid_track_id(slug) {
                return Ok(Self(value));
            }
        }
        Err(ValidationError::InvalidTrackBranch(value))
    }
}

impl AsRef<str> for TrackBranch {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TrackBranch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated non-empty string (trimmed, rejects empty/whitespace-only).
///
/// Used for fields like track title and task description where empty values
/// are semantically invalid. Leading/trailing whitespace is stripped before
/// validation, mirroring the previous `nutype(sanitize(trim), ...)` behaviour.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NonEmptyString(String);

impl NonEmptyString {
    /// Validate and wrap `value` as a [`NonEmptyString`].
    ///
    /// The input is trimmed before validation. Whitespace-only values are
    /// rejected as [`ValidationError::EmptyString`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when the trimmed input is
    /// empty.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let trimmed = value.into().trim().to_owned();
        if trimmed.is_empty() { Err(ValidationError::EmptyString) } else { Ok(Self(trimmed)) }
    }
}

impl AsRef<str> for NonEmptyString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NonEmptyString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated review group name (non-empty, trimmed).
///
/// Used for review group identifiers (e.g., "infra-domain", "usecase-cli").
/// Leading/trailing whitespace is stripped before validation, mirroring the
/// previous `nutype(sanitize(trim), ...)` behaviour.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ReviewGroupName(String);

impl ReviewGroupName {
    /// Validate and wrap `value` as a [`ReviewGroupName`].
    ///
    /// The input is trimmed before validation. Whitespace-only values are
    /// rejected as [`ValidationError::EmptyString`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when the trimmed input is
    /// empty.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let trimmed = value.into().trim().to_owned();
        if trimmed.is_empty() { Err(ValidationError::EmptyString) } else { Ok(Self(trimmed)) }
    }
}

impl AsRef<str> for ReviewGroupName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ReviewGroupName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn is_valid_track_id(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => return false,
    }

    let mut previous_was_hyphen = false;
    for ch in chars {
        let is_valid = ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-';
        if !is_valid {
            return false;
        }
        if ch == '-' && previous_was_hyphen {
            return false;
        }
        previous_was_hyphen = ch == '-';
    }

    !value.ends_with('-')
}

fn is_valid_task_id(value: &str) -> bool {
    let Some(digits) = value.strip_prefix('T') else {
        return false;
    };
    // Must have at least one digit, all digits, and fit in u64
    !digits.is_empty()
        && digits.chars().all(|ch| ch.is_ascii_digit())
        && digits.parse::<u64>().is_ok()
}

fn is_valid_commit_hash(value: &str) -> bool {
    let len = value.len();
    (7..=40).contains(&len)
        && value.chars().all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_track_id_accepts_lowercase_slug() {
        let id = TrackId::try_new("my-feature-2026-04-18").unwrap();
        assert_eq!(id.as_ref(), "my-feature-2026-04-18");
    }

    #[test]
    fn test_track_id_rejects_uppercase() {
        assert!(matches!(
            TrackId::try_new("My-feature").unwrap_err(),
            ValidationError::InvalidTrackId(_)
        ));
    }

    #[test]
    fn test_track_id_rejects_trailing_hyphen() {
        assert!(TrackId::try_new("foo-").is_err());
    }

    #[test]
    fn test_track_id_rejects_double_hyphen() {
        assert!(TrackId::try_new("foo--bar").is_err());
    }

    #[test]
    fn test_task_id_accepts_t_digits() {
        let id = TaskId::try_new("T001").unwrap();
        assert_eq!(id.as_ref(), "T001");
    }

    #[test]
    fn test_task_id_rejects_without_t_prefix() {
        assert!(TaskId::try_new("001").is_err());
    }

    #[test]
    fn test_task_id_rejects_non_digits_after_t() {
        assert!(TaskId::try_new("Tabc").is_err());
    }

    #[test]
    fn test_commit_hash_accepts_short_and_full() {
        assert!(CommitHash::try_new("a1b2c3d").is_ok());
        assert!(CommitHash::try_new("0123456789abcdef0123456789abcdef01234567").is_ok());
    }

    #[test]
    fn test_commit_hash_rejects_too_short() {
        assert!(CommitHash::try_new("abc").is_err());
    }

    #[test]
    fn test_commit_hash_rejects_uppercase() {
        assert!(CommitHash::try_new("A1B2C3D").is_err());
    }

    #[test]
    fn test_track_branch_accepts_valid_branch() {
        let b = TrackBranch::try_new("track/my-feature").unwrap();
        assert_eq!(b.as_ref(), "track/my-feature");
    }

    #[test]
    fn test_track_branch_rejects_wrong_prefix() {
        assert!(TrackBranch::try_new("branch/my-feature").is_err());
    }

    #[test]
    fn test_track_branch_rejects_invalid_slug() {
        assert!(TrackBranch::try_new("track/Foo").is_err());
    }

    #[test]
    fn test_non_empty_string_valid() {
        let result = NonEmptyString::try_new("hello");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_ref(), "hello");
    }

    #[test]
    fn test_non_empty_string_empty_rejected() {
        let result = NonEmptyString::try_new("");
        assert!(matches!(result, Err(ValidationError::EmptyString)));
    }

    #[test]
    fn test_non_empty_string_whitespace_only_rejected() {
        let result = NonEmptyString::try_new("   ");
        assert!(matches!(result, Err(ValidationError::EmptyString)));
    }

    #[test]
    fn test_non_empty_string_trims_whitespace() {
        let result = NonEmptyString::try_new("  hello world  ");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_ref(), "hello world");
    }

    #[test]
    fn test_non_empty_string_display() {
        let s = NonEmptyString::try_new("track title").unwrap();
        assert_eq!(s.to_string(), "track title");
    }

    #[test]
    fn test_review_group_name_valid() {
        let n = ReviewGroupName::try_new("infra-domain").unwrap();
        assert_eq!(n.as_ref(), "infra-domain");
    }

    #[test]
    fn test_review_group_name_whitespace_rejected() {
        assert!(ReviewGroupName::try_new("   ").is_err());
    }

    #[test]
    fn test_review_group_name_trims() {
        let n = ReviewGroupName::try_new("  usecase-cli  ").unwrap();
        assert_eq!(n.as_ref(), "usecase-cli");
    }

    #[test]
    fn test_try_new_accepts_string_and_str() {
        let from_str = TrackId::try_new("abc");
        let from_string = TrackId::try_new(String::from("abc"));
        assert!(from_str.is_ok());
        assert!(from_string.is_ok());
    }
}
