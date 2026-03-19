//! Validated UTC timestamp backed by `chrono::DateTime<Utc>`.

use std::fmt;

use crate::ValidationError;

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
