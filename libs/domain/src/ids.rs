use std::fmt;

use crate::ValidationError;

/// A validated track identifier (lowercase slug format).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(String);

impl TrackId {
    /// Creates a new `TrackId` from the given value.
    ///
    /// # Errors
    /// Returns `ValidationError::InvalidTrackId` if the value is not a valid lowercase slug.
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if is_valid_track_id(&value) {
            Ok(Self(value))
        } else {
            Err(ValidationError::InvalidTrackId(value))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Returns the underlying string.
impl fmt::Display for TrackId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A validated task identifier (format: `T` followed by one or more digits).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(String);

impl TaskId {
    /// Creates a new `TaskId` from the given value.
    ///
    /// # Errors
    /// Returns `ValidationError::InvalidTaskId` if the value does not match `T<digits>`.
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if is_valid_task_id(&value) {
            Ok(Self(value))
        } else {
            Err(ValidationError::InvalidTaskId(value))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A validated git commit hash (7–40 lowercase hex characters).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommitHash(String);

impl CommitHash {
    /// Creates a new `CommitHash` from the given value.
    ///
    /// # Errors
    /// Returns `ValidationError::InvalidCommitHash` if the value is not 7–40 lowercase hex chars.
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if is_valid_commit_hash(&value) {
            Ok(Self(value))
        } else {
            Err(ValidationError::InvalidCommitHash(value))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommitHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A validated track branch name (format: `track/<valid-track-id>`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackBranch(String);

impl TrackBranch {
    /// Branch name prefix.
    const PREFIX: &str = "track/";

    /// Creates a new `TrackBranch` from the given value.
    ///
    /// # Errors
    /// Returns `ValidationError::InvalidTrackBranch` if the value does not match `track/<slug>`.
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if let Some(slug) = value.strip_prefix(Self::PREFIX) {
            if is_valid_track_id(slug) {
                return Ok(Self(value));
            }
        }
        Err(ValidationError::InvalidTrackBranch(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TrackBranch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A validated non-empty string (trimmed, rejects empty/whitespace-only).
///
/// Used for fields like track title and task description where empty values
/// are semantically invalid.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NonEmptyString(String);

impl NonEmptyString {
    /// Creates a new `NonEmptyString`, trimming whitespace.
    ///
    /// # Errors
    /// Returns `ValidationError::EmptyString` if the value is empty or whitespace-only after trimming.
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let trimmed = value.into().trim().to_owned();
        if trimmed.is_empty() { Err(ValidationError::EmptyString) } else { Ok(Self(trimmed)) }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NonEmptyString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
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

/// A validated UTC timestamp backed by `chrono::DateTime<Utc>`.
///
/// Stores both the parsed `DateTime<Utc>` and the original RFC 3339 string
/// so that `as_str()` can return `&str` without allocation.
///
/// Equality, ordering, and hashing are based on the parsed `DateTime<Utc>` only,
/// so two strings representing the same instant (e.g., `+00:00` vs `Z`) compare equal.
#[derive(Debug, Clone)]
pub struct Timestamp {
    dt: chrono::DateTime<chrono::Utc>,
    raw: String,
}

impl PartialEq for Timestamp {
    fn eq(&self, other: &Self) -> bool {
        self.dt == other.dt
    }
}

impl Eq for Timestamp {}

impl std::hash::Hash for Timestamp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.dt.hash(state);
    }
}

impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timestamp {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.dt.cmp(&other.dt)
    }
}

impl Timestamp {
    /// Creates a new `Timestamp` by parsing an ISO 8601 / RFC 3339 string.
    ///
    /// # Errors
    /// Returns `ValidationError::InvalidTimestamp` if the value cannot be parsed.
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let raw = value.into();
        let dt = raw
            .parse::<chrono::DateTime<chrono::Utc>>()
            .map_err(|_| ValidationError::InvalidTimestamp(raw.clone()))?;
        Ok(Self { dt, raw })
    }

    /// Returns the original RFC 3339 string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Returns the underlying `chrono::DateTime<Utc>`.
    #[must_use]
    pub fn as_datetime(&self) -> &chrono::DateTime<chrono::Utc> {
        &self.dt
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_non_empty_string_valid() {
        let result = NonEmptyString::new("hello");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "hello");
    }

    #[test]
    fn test_non_empty_string_empty_rejected() {
        let result = NonEmptyString::new("");
        assert!(matches!(result, Err(ValidationError::EmptyString)));
    }

    #[test]
    fn test_non_empty_string_whitespace_only_rejected() {
        let result = NonEmptyString::new("   ");
        assert!(matches!(result, Err(ValidationError::EmptyString)));
    }

    #[test]
    fn test_non_empty_string_trims_whitespace() {
        let result = NonEmptyString::new("  hello world  ");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "hello world");
    }

    #[test]
    fn test_non_empty_string_display() {
        let s = NonEmptyString::new("track title").unwrap();
        assert_eq!(s.to_string(), "track title");
    }

    // --- Timestamp tests ---

    #[test]
    fn test_timestamp_valid_iso8601() {
        let ts = Timestamp::new("2026-03-19T12:00:00Z").unwrap();
        assert_eq!(ts.as_str(), "2026-03-19T12:00:00Z");
    }

    #[test]
    fn test_timestamp_valid_with_offset() {
        let ts = Timestamp::new("2026-03-19T12:00:00+09:00");
        assert!(ts.is_ok());
    }

    #[test]
    fn test_timestamp_empty_rejected() {
        let result = Timestamp::new("");
        assert!(matches!(result, Err(ValidationError::InvalidTimestamp(_))));
    }

    #[test]
    fn test_timestamp_invalid_format_rejected() {
        let result = Timestamp::new("not-a-timestamp");
        assert!(matches!(result, Err(ValidationError::InvalidTimestamp(_))));
    }

    #[test]
    fn test_timestamp_invalid_date_rejected() {
        let result = Timestamp::new("2026-02-30T12:00:00Z");
        assert!(matches!(result, Err(ValidationError::InvalidTimestamp(_))));
    }

    #[test]
    fn test_timestamp_display() {
        let ts = Timestamp::new("2026-03-19T12:00:00Z").unwrap();
        assert_eq!(ts.to_string(), "2026-03-19T12:00:00Z");
    }

    #[test]
    fn test_timestamp_as_datetime_returns_chrono_type() {
        use chrono::Datelike;
        let ts = Timestamp::new("2026-03-19T12:00:00Z").unwrap();
        assert_eq!(ts.as_datetime().year(), 2026);
    }

    #[test]
    fn test_timestamp_equality_is_time_based_not_string_based() {
        let utc = Timestamp::new("2026-03-19T03:00:00Z").unwrap();
        let offset = Timestamp::new("2026-03-19T12:00:00+09:00").unwrap();
        assert_eq!(utc, offset, "same instant with different representations must be equal");
    }

    #[test]
    fn test_timestamp_hash_is_time_based() {
        use std::collections::HashSet;
        let utc = Timestamp::new("2026-03-19T03:00:00Z").unwrap();
        let offset = Timestamp::new("2026-03-19T12:00:00+09:00").unwrap();
        let mut set = HashSet::new();
        set.insert(utc);
        assert!(set.contains(&offset), "same instant must hash equally");
    }
}
