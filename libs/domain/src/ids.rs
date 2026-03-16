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
