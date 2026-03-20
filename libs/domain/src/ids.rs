use nutype::nutype;

use crate::ValidationError;

fn validate_track_id(value: &str) -> Result<(), ValidationError> {
    if is_valid_track_id(value) {
        Ok(())
    } else {
        Err(ValidationError::InvalidTrackId(value.to_owned()))
    }
}

fn validate_task_id(value: &str) -> Result<(), ValidationError> {
    if is_valid_task_id(value) {
        Ok(())
    } else {
        Err(ValidationError::InvalidTaskId(value.to_owned()))
    }
}

fn validate_commit_hash(value: &str) -> Result<(), ValidationError> {
    if is_valid_commit_hash(value) {
        Ok(())
    } else {
        Err(ValidationError::InvalidCommitHash(value.to_owned()))
    }
}

fn validate_track_branch(value: &str) -> Result<(), ValidationError> {
    if let Some(slug) = value.strip_prefix("track/") {
        if is_valid_track_id(slug) {
            return Ok(());
        }
    }
    Err(ValidationError::InvalidTrackBranch(value.to_owned()))
}

fn validate_non_empty(value: &str) -> Result<(), ValidationError> {
    if value.is_empty() { Err(ValidationError::EmptyString) } else { Ok(()) }
}

/// A validated track identifier (lowercase slug format).
#[nutype(
    validate(with = validate_track_id, error = ValidationError),
    derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, AsRef)
)]
pub struct TrackId(String);

/// A validated task identifier (format: `T` followed by one or more digits).
#[nutype(
    validate(with = validate_task_id, error = ValidationError),
    derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, AsRef)
)]
pub struct TaskId(String);

/// A validated git commit hash (7–40 lowercase hex characters).
#[nutype(
    validate(with = validate_commit_hash, error = ValidationError),
    derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, AsRef)
)]
pub struct CommitHash(String);

/// A validated track branch name (format: `track/<valid-track-id>`).
#[nutype(
    validate(with = validate_track_branch, error = ValidationError),
    derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, AsRef)
)]
pub struct TrackBranch(String);

/// A validated non-empty string (trimmed, rejects empty/whitespace-only).
///
/// Used for fields like track title and task description where empty values
/// are semantically invalid.
#[nutype(
    sanitize(trim),
    validate(with = validate_non_empty, error = ValidationError),
    derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, AsRef)
)]
pub struct NonEmptyString(String);

/// A validated review group name (non-empty, trimmed).
///
/// Used for review group identifiers (e.g., "infra-domain", "usecase-cli").
#[nutype(
    sanitize(trim),
    validate(with = validate_non_empty, error = ValidationError),
    derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, AsRef)
)]
pub struct ReviewGroupName(String);

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
}
