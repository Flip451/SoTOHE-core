//! Review concern types: slugified concern names and streak tracking.

use std::fmt;

use super::error::ReviewError;
use super::types::RoundType;
use crate::Timestamp;

/// A validated concern slug used for escalation tracking.
///
/// Concern slugs are lowercase, trimmed, non-empty strings.
/// The reserved value "other" is allowed (it is the fallback concern).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewConcern(String);

impl ReviewConcern {
    /// Creates a new validated `ReviewConcern`.
    ///
    /// # Errors
    /// Returns `ReviewError::InvalidConcern` if `s` is empty after trimming.
    pub fn try_new(s: impl AsRef<str>) -> Result<Self, ReviewError> {
        let s = s.as_ref().trim().to_lowercase();
        if s.is_empty() {
            return Err(ReviewError::InvalidConcern("concern slug must not be empty".to_owned()));
        }
        Ok(Self(s))
    }
}

impl AsRef<str> for ReviewConcern {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ReviewConcern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Escalation streak for a single concern across review rounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewConcernStreak {
    consecutive_rounds: u8,
    last_round_type: RoundType,
    last_round: u32,
    last_seen_at: Timestamp,
}

impl ReviewConcernStreak {
    /// Creates a new streak with the given values.
    #[must_use]
    pub fn new(
        consecutive_rounds: u8,
        last_round_type: RoundType,
        last_round: u32,
        last_seen_at: Timestamp,
    ) -> Self {
        Self { consecutive_rounds, last_round_type, last_round, last_seen_at }
    }

    /// Returns the number of consecutive rounds this concern has appeared.
    #[must_use]
    pub fn consecutive_rounds(&self) -> u8 {
        self.consecutive_rounds
    }

    /// Returns the round type of the last round this concern appeared.
    #[must_use]
    pub fn last_round_type(&self) -> RoundType {
        self.last_round_type
    }

    /// Returns the round number of the last round this concern appeared.
    #[must_use]
    pub fn last_round(&self) -> u32 {
        self.last_round
    }

    /// Returns the timestamp of the last round this concern appeared.
    #[must_use]
    pub fn last_seen_at(&self) -> &str {
        self.last_seen_at.as_str()
    }
}

/// Summary of a review cycle for escalation tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCycleSummary {
    round_type: RoundType,
    round: u32,
    timestamp: Timestamp,
    concerns: Vec<ReviewConcern>,
    groups: Vec<crate::ReviewGroupName>,
}

impl ReviewCycleSummary {
    /// Creates a new cycle summary.
    #[must_use]
    pub fn new(
        round_type: RoundType,
        round: u32,
        timestamp: Timestamp,
        concerns: Vec<ReviewConcern>,
        groups: Vec<crate::ReviewGroupName>,
    ) -> Self {
        Self { round_type, round, timestamp, concerns, groups }
    }

    /// Returns the round type.
    #[must_use]
    pub fn round_type(&self) -> RoundType {
        self.round_type
    }

    /// Returns the round number.
    #[must_use]
    pub fn round(&self) -> u32 {
        self.round
    }

    /// Returns the timestamp string.
    #[must_use]
    pub fn timestamp(&self) -> &str {
        self.timestamp.as_str()
    }

    /// Returns the concerns for this cycle.
    #[must_use]
    pub fn concerns(&self) -> &[ReviewConcern] {
        &self.concerns
    }

    /// Returns the groups for this cycle.
    #[must_use]
    pub fn groups(&self) -> &[crate::ReviewGroupName] {
        &self.groups
    }
}

/// Converts a file path to a concern slug.
///
/// Takes the path components after removing `libs/`, `apps/`, etc. prefixes
/// and joins them with dots.
#[must_use]
pub fn file_path_to_concern(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        return String::new();
    }

    // Strip common prefixes
    let stripped = if let Some(rest) = path.strip_prefix("libs/") {
        rest
    } else if let Some(rest) = path.strip_prefix("apps/") {
        rest
    } else {
        path
    };

    // Collect all meaningful path components (skip "src", lowercase, strip .rs extension).
    let parts: Vec<String> = stripped
        .split('/')
        .map(|s| {
            let s = if let Some(stem) = s.strip_suffix(".rs") { stem } else { s };
            s.to_lowercase()
        })
        .filter(|s| s != "src" && !s.is_empty())
        .collect();

    parts.join(".")
}
